//! Adapts the independent parser/grid and native transport to Termy's public
//! terminal API. Renderer callbacks borrow one coherent engine state.

use super::{
    MAX_TERMINAL_SCROLLBACK_HISTORY, TabTitleShellIntegration, TerminalCursorState,
    TerminalCursorStyle, TerminalDamageSnapshot, TerminalDirtySpan, TerminalEvent, TerminalLaunch,
    TerminalOptions, TerminalRuntimeConfig, TerminalSize, TerminalWakeupNotifier,
    resolve_launch_working_directory, resolve_terminal_launch, terminal_environment_overrides,
};
use crate::{
    DetectedLink, DetectedViewportLink, KittyClipboardControl, KittyClipboardHostState,
    KittyClipboardInput, KittyClipboardInterceptor, KittyClipboardOsc,
    KittyGraphicsRenderPlacement, TerminalClipboardLocation, TerminalClipboardTarget,
    TerminalColor, TerminalKeyboardMode, TerminalMouseMode, TerminalPalette, TerminalQueryColors,
    TerminalRenderCell, TerminalRenderColor, TerminalRenderDamageSnapshot, TerminalRenderRead,
    TerminalRenderText, TerminalReplyHost, TerminalUnderlineStyle, TerminalViewportMetadata,
    TermyCell, TermyColor, TermyFrame, TermyFrameUpdate, TermySearchMatch, TermySearchOptions,
    TermySharedSearchMatch,
    search::search_lines_shared,
    terminal_engine::transport::{PtySize, SpawnConfig, Transport},
    terminal_engine::{self as engine, Engine},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD as BASE64, STANDARD_NO_PAD},
};
use flume::Sender;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Condvar, Mutex, MutexGuard, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

const EVENT_BATCH: usize = 2048;
const MAX_EVENTS: usize = 65_536;
const MAX_PENDING_REPLIES: usize = 2 * 1024 * 1024;
const PARSE_BATCH: usize = 4096;

pub(super) struct CustomBackend {
    shared: Arc<Shared>,
    transport: Option<Arc<Transport>>,
}

struct Shared {
    state: Mutex<State>,
    clipboard: Mutex<KittyClipboardHostState>,
    notifier: Option<TerminalWakeupNotifier>,
    wakeup_enabled: AtomicBool,
    wakeup_queued: AtomicBool,
    transport: Mutex<Weak<Transport>>,
    sync_signal: Arc<SyncSignal>,
    sync_watchdog_started: AtomicBool,
}

#[derive(Default)]
struct SyncSignal {
    state: Mutex<SyncSignalState>,
    changed: Condvar,
}

#[derive(Default)]
struct SyncSignalState {
    deadline: Option<Instant>,
    shutdown: bool,
}

impl Drop for Shared {
    fn drop(&mut self) {
        self.sync_signal
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .shutdown = true;
        self.sync_signal.changed.notify_one();
    }
}

struct State {
    engine: Engine,
    clipboard_interceptor: KittyClipboardInterceptor,
    clipboard_paste_events: bool,
    size: TerminalSize,
    query_colors: TerminalQueryColors,
    default_cursor_style: TerminalCursorStyle,
    events: VecDeque<PendingEvent>,
    replies: Vec<u8>,
    generation: u64,
    palette_epoch: u64,
    force_full_damage: bool,
    last_damage_cursor: Option<TerminalCursorState>,
}

enum PendingEvent {
    Terminal(TerminalEvent),
    ClipboardLoad(String),
    Kitty(KittyClipboardOsc),
    KittyControl(KittyClipboardControl),
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn notify(&self) {
        if !self.wakeup_enabled.load(Ordering::Acquire) {
            return;
        }
        if !self.wakeup_queued.swap(true, Ordering::AcqRel) {
            crate::render_metrics::increment_runtime_wakeup_count();
            if let Some(notifier) = &self.notifier {
                notifier.notify();
            }
        }
    }

    fn feed(self: &Arc<Self>, bytes: &[u8], hydrate: bool) -> Vec<u8> {
        if bytes.is_empty() {
            return Vec::new();
        }
        let mut state = self.state();
        let replies = state.feed(bytes, hydrate);
        let should_notify = !state.engine.modes().synchronized_update || !state.events.is_empty();
        let deadline = state.engine.synchronized_update_deadline();
        drop(state);
        self.schedule_sync_timeout(deadline);
        if should_notify {
            self.notify();
        }
        replies
    }
    fn schedule_sync_timeout(self: &Arc<Self>, deadline: Option<Instant>) {
        let mut signal = self
            .sync_signal
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if signal.deadline == deadline {
            return;
        }
        signal.deadline = deadline;
        self.sync_signal.changed.notify_one();
        drop(signal);
        if deadline.is_none() || self.sync_watchdog_started.swap(true, Ordering::AcqRel) {
            return;
        }
        let shared = Arc::downgrade(self);
        let signal = self.sync_signal.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("termy-sync-watchdog".into())
            .spawn(move || {
                loop {
                    let mut pending = signal
                        .state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    loop {
                        if pending.shutdown {
                            return;
                        }
                        let Some(deadline) = pending.deadline else {
                            pending = signal
                                .changed
                                .wait(pending)
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            continue;
                        };
                        let now = Instant::now();
                        if now < deadline {
                            let (next, _) = signal
                                .changed
                                .wait_timeout(pending, deadline - now)
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            pending = next;
                            continue;
                        }
                        pending.deadline = None;
                        drop(pending);
                        let Some(shared) = shared.upgrade() else {
                            return;
                        };
                        let mut state = shared.state();
                        if state.engine.synchronized_update_deadline() != Some(deadline) {
                            break;
                        }
                        let committed = state.engine.stop_synchronized_update();
                        let mut replies = Vec::new();
                        if committed {
                            while let Some(event) = state.engine.pop_event() {
                                state.engine_event(event);
                            }
                            state.engine.drain_replies(&mut replies);
                            state.generation = state.generation.wrapping_add(1);
                        }
                        drop(state);
                        if !replies.is_empty() {
                            let transport = shared
                                .transport
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .upgrade();
                            if let Some(transport) = transport {
                                if let Err(error) = transport.write_protocol_reply_owned(replies) {
                                    log::warn!("terminal timeout reply failed: {error}");
                                }
                            } else {
                                shared.state().append_replies(&replies);
                            }
                        }
                        if committed {
                            shared.notify();
                        }
                        break;
                    }
                }
            })
        {
            self.sync_watchdog_started.store(false, Ordering::Release);
            log::warn!("could not start terminal synchronization watchdog: {error}");
        }
    }
}

impl State {
    fn new(size: TerminalSize, config: &TerminalRuntimeConfig) -> Self {
        let mut engine = Engine::new(
            engine_size(size.clamped()),
            engine::Options {
                scrollback_history: config
                    .scrollback_history
                    .min(MAX_TERMINAL_SCROLLBACK_HISTORY),
            },
        );
        engine.set_default_cursor_shape(cursor_shape(config.default_cursor_style));
        engine.set_query_colors(config.query_colors);
        engine.set_cell_pixels(size.cell_width, size.cell_height);
        let actual = engine.size();
        Self {
            engine,
            clipboard_interceptor: KittyClipboardInterceptor::default(),
            clipboard_paste_events: false,
            size: TerminalSize {
                cols: actual.cols as u16,
                rows: actual.rows as u16,
                ..size.clamped()
            },
            query_colors: config.query_colors,
            default_cursor_style: config.default_cursor_style,
            events: VecDeque::new(),
            replies: Vec::new(),
            generation: 0,
            palette_epoch: 0,
            force_full_damage: true,
            last_damage_cursor: None,
        }
    }

    fn queue(&mut self, event: PendingEvent) {
        if self.events.len() < MAX_EVENTS {
            self.events.push_back(event);
        } else if matches!(event, PendingEvent::Terminal(TerminalEvent::Exit)) {
            self.events.pop_front();
            self.events.push_back(event);
        }
    }

    fn feed(&mut self, bytes: &[u8], hydrate: bool) -> Vec<u8> {
        let before_generation = self.engine.generation();
        let mut replies = Vec::new();
        for bytes in bytes.chunks(PARSE_BATCH) {
            let (filtered, clipboard) = self.clipboard_interceptor.process(bytes);
            for event in clipboard {
                match event {
                    KittyClipboardInput::Packet(packet) => {
                        if !hydrate {
                            self.queue(PendingEvent::Kitty(packet));
                        }
                    }
                    KittyClipboardInput::Control(control) => {
                        match control {
                            KittyClipboardControl::Set(enabled) => {
                                self.clipboard_paste_events = enabled;
                            }
                            KittyClipboardControl::Reset => self.clipboard_paste_events = false,
                            KittyClipboardControl::Query => {
                                if !hydrate {
                                    let enabled = if self.clipboard_paste_events { 1 } else { 2 };
                                    replies.extend_from_slice(
                                        format!("\x1b[?5522;{enabled}$y").as_bytes(),
                                    );
                                }
                            }
                        }
                        if !hydrate {
                            self.queue(PendingEvent::KittyControl(control));
                        }
                    }
                }
            }
            self.engine.feed(&filtered);
            while let Some(event) = self.engine.pop_event() {
                if !hydrate {
                    self.engine_event(event);
                }
            }
            self.engine.drain_replies(&mut replies);
            if hydrate {
                replies.clear();
            }
        }
        if hydrate {
            self.engine.stop_synchronized_update();
            while self.engine.pop_event().is_some() {}
            self.engine.drain_replies(&mut replies);
            replies.clear();
        }
        if self.engine.generation() != before_generation {
            self.generation = self.generation.wrapping_add(1);
        }
        replies
    }

    fn engine_event(&mut self, event: engine::Event) {
        let event = match event {
            engine::Event::Bell => TerminalEvent::Bell,
            engine::Event::Title(title) => TerminalEvent::Title(title),
            engine::Event::ResetTitle => TerminalEvent::ResetTitle,
            engine::Event::Progress(progress) => TerminalEvent::Progress(progress),
            engine::Event::WorkingDirectory(path) => TerminalEvent::WorkingDirectory(path),
            engine::Event::ShellIntegration(value) => {
                let mut parts = value.split(';');
                match parts.next() {
                    Some("A") => TerminalEvent::ShellPromptStart,
                    Some("B") => TerminalEvent::ShellCommandStart,
                    Some("C") => TerminalEvent::ShellCommandExecuting,
                    Some("D") => TerminalEvent::ShellCommandFinished(
                        parts.next().and_then(|code| code.parse().ok()),
                    ),
                    _ => return,
                }
            }
            engine::Event::Clipboard { selection, data } => {
                if data == "?" {
                    self.queue(PendingEvent::ClipboardLoad(selection));
                    return;
                }
                let Ok(bytes) = STANDARD_NO_PAD
                    .decode(data.trim_end_matches('=').as_bytes())
                    .or_else(|_| BASE64.decode(&data))
                else {
                    return;
                };
                let Ok(text) = String::from_utf8(bytes) else {
                    return;
                };
                TerminalEvent::ClipboardStore(text)
            }
        };
        self.queue(PendingEvent::Terminal(event));
    }

    fn append_replies(&mut self, bytes: &[u8]) {
        let available = MAX_PENDING_REPLIES.saturating_sub(self.replies.len());
        if bytes.len() <= available {
            self.replies.extend_from_slice(bytes);
        } else {
            log::warn!("terminal protocol reply queue is full");
        }
    }

    fn cursor(&self) -> Option<TerminalCursorState> {
        let cursor = self.engine.cursor();
        let row = cursor.row.saturating_add(self.engine.display_offset());
        (cursor.visible && row < self.engine.size().rows).then_some(TerminalCursorState {
            row,
            col: cursor.col,
            style: match cursor.shape {
                engine::CursorShape::Block => TerminalCursorStyle::Block,
                engine::CursorShape::Beam | engine::CursorShape::Underline => {
                    TerminalCursorStyle::Line
                }
            },
        })
    }

    fn metadata(&self) -> TerminalViewportMetadata {
        TerminalViewportMetadata {
            cols: self.size.cols,
            rows: self.size.rows,
            cursor: self.cursor(),
            display_offset: self.engine.display_offset(),
            history_size: self.engine.history_size(),
            palette_revision: self
                .engine
                .palette_revision()
                .wrapping_add(self.palette_epoch),
            generation: self.generation,
        }
    }

    fn palette(&self) -> TerminalPalette {
        TerminalPalette {
            indexed: std::array::from_fn(|index| self.engine.palette()[index].and_then(rgb)),
            foreground: self.engine.foreground().and_then(rgb),
            background: self.engine.background().and_then(rgb),
            cursor: self.engine.cursor_color().and_then(rgb),
            revision: self
                .engine
                .palette_revision()
                .wrapping_add(self.palette_epoch),
        }
    }

    fn take_damage(&mut self, force_full: bool) -> TerminalRenderDamageSnapshot {
        let source = self.engine.take_damage();
        let pending_full = std::mem::take(&mut self.force_full_damage);
        let full = force_full || pending_full;
        let mut damage = if full {
            TerminalDamageSnapshot::Full
        } else {
            match source {
                engine::Damage::Full => TerminalDamageSnapshot::Full,
                engine::Damage::Partial(spans) => TerminalDamageSnapshot::Partial(
                    spans
                        .into_iter()
                        .filter(|span| span.start < span.end)
                        .map(|span| TerminalDirtySpan {
                            row: span.row,
                            left_col: span.start.saturating_sub(1),
                            right_col: span.end.min(self.engine.size().cols.saturating_sub(1)),
                        })
                        .collect(),
                ),
            }
        };
        let cursor = self.cursor();
        if cursor != self.last_damage_cursor {
            if let TerminalDamageSnapshot::Partial(spans) = &mut damage {
                for cursor in self.last_damage_cursor.into_iter().chain(cursor) {
                    if cursor.row < self.engine.size().rows {
                        spans.push(TerminalDirtySpan {
                            row: cursor.row,
                            left_col: cursor.col.saturating_sub(1),
                            right_col: cursor
                                .col
                                .saturating_add(1)
                                .min(self.engine.size().cols - 1),
                        });
                    }
                }
                normalize_spans(spans);
            }
            self.last_damage_cursor = cursor;
        }
        TerminalRenderDamageSnapshot {
            damage,
            scrolls: Vec::new(),
            generation: self.generation,
            palette_revision: self.metadata().palette_revision,
        }
    }

    fn render_read(&mut self, force_full: bool) -> TerminalRenderRead {
        let update = self.take_damage(force_full);
        let mut cells =
            Vec::with_capacity(usize::from(self.size.cols) * usize::from(self.size.rows));
        for row in 0..usize::from(self.size.rows) {
            let wrapped = self.engine.viewport_row_wrapped(row);
            if let Some(source) = self.engine.viewport_row(row) {
                for (col, cell) in source.iter().enumerate() {
                    cells.push(render_cell(cell, wrapped && col + 1 == source.len()));
                }
            }
        }
        TerminalRenderRead {
            metadata: self.metadata(),
            palette: self.palette(),
            cells,
            update,
        }
    }
}

impl CustomBackend {
    pub(super) fn new(
        size: TerminalSize,
        working_dir: Option<&str>,
        wakeup: Option<Sender<()>>,
        shell_integration: Option<&TabTitleShellIntegration>,
        config: Option<&TerminalRuntimeConfig>,
        startup: Option<&str>,
    ) -> anyhow::Result<Self> {
        let notifier = wakeup.map(|sender| {
            TerminalWakeupNotifier::new(move || {
                let _ = sender.try_send(());
            })
        });
        Self::new_with_wakeup_notifier(
            size,
            working_dir,
            notifier,
            shell_integration,
            config,
            startup,
        )
    }

    pub(super) fn new_with_wakeup_notifier(
        size: TerminalSize,
        working_dir: Option<&str>,
        notifier: Option<TerminalWakeupNotifier>,
        shell_integration: Option<&TabTitleShellIntegration>,
        config: Option<&TerminalRuntimeConfig>,
        startup: Option<&str>,
    ) -> anyhow::Result<Self> {
        let launch = startup.map(|command| TerminalLaunch::ShellCommand(command.to_owned()));
        Self::new_with_launch_and_wakeup_notifier(
            size,
            working_dir,
            notifier,
            shell_integration,
            config,
            launch.as_ref(),
        )
    }

    pub(super) fn new_with_launch_and_wakeup_notifier(
        size: TerminalSize,
        working_dir: Option<&str>,
        notifier: Option<TerminalWakeupNotifier>,
        shell_integration: Option<&TabTitleShellIntegration>,
        config: Option<&TerminalRuntimeConfig>,
        launch: Option<&TerminalLaunch>,
    ) -> anyhow::Result<Self> {
        if !engine::transport::available() {
            anyhow::bail!("the native terminal transport is unavailable on this host");
        }
        let config = config.cloned().unwrap_or_default();
        let mut terminal = Self::new_display_with_wakeup_notifier(size, Some(&config), notifier);
        let launch = resolve_terminal_launch(&config, launch)?;
        let spawn = SpawnConfig {
            program: launch.program,
            args: launch.args,
            working_directory: resolve_launch_working_directory(
                working_dir,
                config.working_dir_fallback,
            ),
            environment: terminal_environment_overrides(shell_integration, &config)
                .into_iter()
                .collect(),
        };
        let shared = terminal.shared.clone();
        let exit = terminal.shared.clone();
        terminal.transport = Some(Arc::new(Transport::spawn(
            spawn,
            pty_size(terminal.size()),
            move |bytes| shared.feed(bytes, false),
            move || {
                let mut state = exit.state();
                if state.engine.stop_synchronized_update() {
                    while let Some(event) = state.engine.pop_event() {
                        state.engine_event(event);
                    }
                    let mut replies = Vec::new();
                    state.engine.drain_replies(&mut replies);
                    state.append_replies(&replies);
                    state.generation = state.generation.wrapping_add(1);
                }
                state.queue(PendingEvent::Terminal(TerminalEvent::Exit));
                drop(state);
                exit.notify();
            },
        )?));
        *terminal
            .shared
            .transport
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Arc::downgrade(terminal.transport.as_ref().unwrap());
        Ok(terminal)
    }

    pub(super) fn new_display(size: TerminalSize, config: Option<&TerminalRuntimeConfig>) -> Self {
        Self::new_display_with_wakeup_notifier(size, config, None)
    }

    pub(super) fn new_display_with_wakeup_notifier(
        size: TerminalSize,
        config: Option<&TerminalRuntimeConfig>,
        notifier: Option<TerminalWakeupNotifier>,
    ) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State::new(size, &config.cloned().unwrap_or_default())),
                clipboard: Mutex::new(KittyClipboardHostState::new()),
                notifier,
                wakeup_enabled: AtomicBool::new(true),
                wakeup_queued: AtomicBool::new(false),
                transport: Mutex::new(Weak::new()),
                sync_signal: Arc::new(SyncSignal::default()),
                sync_watchdog_started: AtomicBool::new(false),
            }),
            transport: None,
        }
    }

    pub(super) fn feed_output(&self, bytes: &[u8]) {
        let replies = self.shared.feed(bytes, false);
        self.send_reply(replies);
    }
    pub(super) fn hydrate_output(&self, bytes: &[u8]) {
        self.shared.feed(bytes, true);
    }
    pub(super) fn child_pid(&self) -> Option<u32> {
        self.transport
            .as_ref()
            .map(|transport| transport.child_pid())
    }
    pub(super) fn set_wakeup_enabled(&self, enabled: bool) {
        let old = self.shared.wakeup_enabled.swap(enabled, Ordering::AcqRel);
        if enabled && !old {
            self.shared.wakeup_queued.store(false, Ordering::Release);
            self.shared.notify();
        }
    }
    pub(super) fn write(&self, input: &[u8]) {
        if let Some(transport) = &self.transport
            && let Err(error) = transport.write(input)
        {
            log::warn!("terminal input failed: {error}");
        }
    }
    pub(super) fn write_owned(&self, input: Vec<u8>) {
        if let Some(transport) = &self.transport
            && let Err(error) = transport.write_owned(input)
        {
            log::warn!("terminal input failed: {error}");
        }
    }
    pub(super) fn write_str(&self, input: &str) {
        self.write(input.as_bytes());
    }
    fn send_reply(&self, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        if let Some(transport) = &self.transport {
            if let Err(error) = transport.write_protocol_reply_owned(bytes) {
                log::warn!("terminal reply failed: {error}");
            }
        } else {
            self.shared.state().append_replies(&bytes);
        }
    }

    pub(super) fn resize(&mut self, size: TerminalSize) {
        let mut state = self.shared.state();
        let size = size.clamped();
        if state.size == size {
            return;
        }
        state.engine.resize(engine_size(size));
        state
            .engine
            .set_cell_pixels(size.cell_width, size.cell_height);
        let actual = state.engine.size();
        let size = TerminalSize {
            cols: actual.cols as u16,
            rows: actual.rows as u16,
            ..size
        };
        state.size = size;
        state.generation = state.generation.wrapping_add(1);
        state.force_full_damage = true;
        while let Some(event) = state.engine.pop_event() {
            state.engine_event(event);
        }
        let mut replies = Vec::new();
        state.engine.drain_replies(&mut replies);
        drop(state);
        self.shared.schedule_sync_timeout(None);
        self.send_reply(replies);
        if let Some(transport) = &self.transport
            && let Err(error) = transport.resize(pty_size(size))
        {
            log::warn!("terminal resize failed: {error}");
        }
        self.shared.notify();
    }
    pub(super) fn nudge_resize(&self) {
        if let Some(transport) = &self.transport
            && let Err(error) = transport.resize(pty_size(self.size()))
        {
            log::warn!("terminal resize notification failed: {error}");
        }
    }
    pub(super) fn size(&self) -> TerminalSize {
        self.shared.state().size
    }

    pub(super) fn kitty_graphics_snapshot(&self) -> (u64, Vec<KittyGraphicsRenderPlacement>) {
        let mut state = self.shared.state();
        state.engine.graphics_snapshot()
    }
    pub(super) fn kitty_graphics_placements(&self) -> Vec<KittyGraphicsRenderPlacement> {
        self.kitty_graphics_snapshot().1
    }
    pub(super) fn kitty_graphics_revision(&self) -> u64 {
        self.shared.state().engine.poll_graphics_revision()
    }
    pub(super) fn kitty_clipboard_paste_events_enabled(&self) -> bool {
        self.shared.state().clipboard_paste_events
    }
    pub(super) fn kitty_clipboard_paste_notification(
        &self,
        location: TerminalClipboardLocation,
        formats: &[String],
    ) -> Option<Vec<u8>> {
        let enabled = self.kitty_clipboard_paste_events_enabled();
        let mut clipboard = self
            .shared
            .clipboard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        clipboard.set_paste_events_enabled(enabled);
        clipboard.paste_notification(location, formats)
    }
    pub(super) fn send_kitty_clipboard_paste_event(
        &self,
        location: TerminalClipboardLocation,
        formats: &[String],
    ) -> bool {
        let Some(reply) = self.kitty_clipboard_paste_notification(location, formats) else {
            return false;
        };
        self.send_reply(reply);
        true
    }

    pub(super) fn drain_events(
        &self,
        host: &mut impl TerminalReplyHost,
    ) -> (Vec<TerminalEvent>, bool) {
        let wakeup = self.shared.wakeup_queued.swap(false, Ordering::AcqRel);
        let (batch, has_more) = {
            let mut state = self.shared.state();
            let count = state.events.len().min(EVENT_BATCH);
            let batch: Vec<_> = state.events.drain(..count).collect();
            (batch, !state.events.is_empty())
        };
        let mut events = Vec::with_capacity(batch.len() + usize::from(wakeup));
        if wakeup {
            events.push(TerminalEvent::Wakeup);
        }
        for event in batch {
            match event {
                PendingEvent::Terminal(event) => events.push(event),
                PendingEvent::ClipboardLoad(selection) => {
                    let target = if selection.contains('c') || selection.is_empty() {
                        TerminalClipboardTarget::Clipboard
                    } else {
                        TerminalClipboardTarget::Selection
                    };
                    if let Some(text) = host.load_clipboard(target) {
                        self.send_reply(
                            format!("\x1b]52;{selection};{}\x1b\\", BASE64.encode(text))
                                .into_bytes(),
                        );
                    }
                }
                PendingEvent::Kitty(packet) => {
                    let replies = self
                        .shared
                        .clipboard
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .handle_osc(packet, host);
                    for reply in replies {
                        self.send_reply(reply);
                    }
                }
                PendingEvent::KittyControl(control) => {
                    let mut clipboard = self
                        .shared
                        .clipboard
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    match control {
                        KittyClipboardControl::Set(enabled) => {
                            clipboard.set_paste_events_enabled(enabled);
                        }
                        KittyClipboardControl::Reset => clipboard.reset(),
                        KittyClipboardControl::Query => {}
                    }
                }
            }
        }
        let replies = {
            let mut state = self.shared.state();
            std::mem::take(&mut state.replies)
        };
        if !replies.is_empty() {
            host.protocol_reply(&replies);
        }
        (events, has_more)
    }
    pub(super) fn has_pending_events(&self) -> bool {
        let state = self.shared.state();
        self.shared.wakeup_queued.load(Ordering::Acquire)
            || !state.events.is_empty()
            || !state.replies.is_empty()
    }
    pub(super) fn set_query_colors(&mut self, colors: TerminalQueryColors) {
        let mut state = self.shared.state();
        state.query_colors = colors;
        state.engine.set_query_colors(colors);
        state.palette_epoch = state.palette_epoch.wrapping_add(1);
        state.force_full_damage = true;
    }
    pub(super) fn palette(&self) -> TerminalPalette {
        self.shared.state().palette()
    }
    pub(super) fn snapshot(&self) -> TermyFrame {
        let state = self.shared.state();
        let palette = state.palette();
        let mut cells =
            Vec::with_capacity(usize::from(state.size.cols) * usize::from(state.size.rows));
        for row in 0..usize::from(state.size.rows) {
            let wrapped = state.engine.viewport_row_wrapped(row);
            if let Some(line) = state.engine.viewport_row(row) {
                for (col, cell) in line.iter().enumerate() {
                    cells.push(legacy_cell(
                        cell,
                        wrapped && col + 1 == line.len(),
                        &palette,
                        state.query_colors,
                    ));
                }
            }
        }
        TermyFrame {
            cols: state.size.cols,
            rows: state.size.rows,
            cells,
            cursor: state.cursor(),
            display_offset: state.engine.display_offset(),
            history_size: state.engine.history_size(),
        }
    }
    pub(super) fn frame_update(&self, force_full: bool) -> TermyFrameUpdate {
        let mut state = self.shared.state();
        let update = state.take_damage(force_full);
        let palette = state.palette();
        let spans = match &update.damage {
            TerminalDamageSnapshot::Full => (0..usize::from(state.size.rows))
                .map(|row| TerminalDirtySpan {
                    row,
                    left_col: 0,
                    right_col: usize::from(state.size.cols) - 1,
                })
                .collect(),
            TerminalDamageSnapshot::Partial(spans) => spans.clone(),
        };
        let mut cells = Vec::new();
        for span in spans {
            let wrapped = state.engine.viewport_row_wrapped(span.row);
            if let Some(line) = state.engine.viewport_row(span.row) {
                let right = span.right_col.min(line.len() - 1);
                for col in span.left_col..=right {
                    cells.push(legacy_cell(
                        &line[col],
                        wrapped && col + 1 == line.len(),
                        &palette,
                        state.query_colors,
                    ));
                }
            }
        }
        TermyFrameUpdate {
            cols: state.size.cols,
            rows: state.size.rows,
            cells,
            cursor: state.cursor(),
            display_offset: state.engine.display_offset(),
            history_size: state.engine.history_size(),
            damage: update.damage,
        }
    }
    pub(super) fn take_render_damage_snapshot(&self) -> TerminalRenderDamageSnapshot {
        self.shared.state().take_damage(false)
    }
    pub(super) fn take_damage_snapshot(&self) -> TerminalDamageSnapshot {
        self.take_render_damage_snapshot().damage
    }
    pub(super) fn render_read(&self, force_full: bool) -> TerminalRenderRead {
        self.shared.state().render_read(force_full)
    }
    pub(super) fn render_read_with_screen(&self, force_full: bool) -> (TerminalRenderRead, bool) {
        let mut state = self.shared.state();
        let alternate = state.engine.alternate_screen();
        (state.render_read(force_full), alternate)
    }
    pub(super) fn visit_viewport_cells(
        &self,
        mut visitor: impl FnMut(usize, i32, usize, &TerminalRenderCell),
    ) -> TerminalViewportMetadata {
        // Preserve reentrant callbacks at the public snapshot boundary. The
        // desktop renderer uses the explicit locked variant below instead.
        let mut cells = Vec::new();
        let metadata = self.visit_viewport_cells_locked(|_, _, _, cell| cells.push(cell.clone()));
        let cols = usize::from(metadata.cols);
        for (index, cell) in cells.iter().enumerate() {
            visitor(
                metadata.display_offset,
                (index / cols) as i32 - metadata.display_offset as i32,
                index % cols,
                cell,
            );
        }
        metadata
    }
    pub(super) fn visit_viewport_cells_locked(
        &self,
        mut visitor: impl FnMut(usize, i32, usize, &TerminalRenderCell),
    ) -> TerminalViewportMetadata {
        let state = self.shared.state();
        let offset = state.engine.display_offset();
        for row in 0..usize::from(state.size.rows) {
            let wrapped = state.engine.viewport_row_wrapped(row);
            if let Some(line) = state.engine.viewport_row(row) {
                for (col, cell) in line.iter().enumerate() {
                    visitor(
                        offset,
                        row as i32 - offset as i32,
                        col,
                        &render_cell(cell, wrapped && col + 1 == line.len()),
                    );
                }
            }
        }
        state.metadata()
    }
    pub(super) fn visit_viewport_ranges_at_generation(
        &self,
        generation: u64,
        spans: &[TerminalDirtySpan],
        mut visitor: impl FnMut(usize, usize, i32, usize, &TerminalRenderCell),
    ) -> bool {
        let mut cells = Vec::new();
        if !self.visit_viewport_ranges_locked_at_generation(
            generation,
            spans,
            |row, offset, line, col, cell| cells.push((row, offset, line, col, cell.clone())),
        ) {
            return false;
        }
        for (row, offset, line, col, cell) in cells {
            visitor(row, offset, line, col, &cell);
        }
        true
    }
    pub(super) fn visit_viewport_ranges_locked_at_generation(
        &self,
        generation: u64,
        spans: &[TerminalDirtySpan],
        mut visitor: impl FnMut(usize, usize, i32, usize, &TerminalRenderCell),
    ) -> bool {
        let state = self.shared.state();
        if state.generation != generation {
            return false;
        }
        let offset = state.engine.display_offset();
        for span in spans {
            let wrapped = state.engine.viewport_row_wrapped(span.row);
            if let Some(line) = state.engine.viewport_row(span.row) {
                let end = span.right_col.saturating_add(1).min(line.len());
                for col in span.left_col.min(end)..end {
                    visitor(
                        span.row,
                        offset,
                        span.row as i32 - offset as i32,
                        col,
                        &render_cell(&line[col], wrapped && col + 1 == line.len()),
                    );
                }
            }
        }
        true
    }
    pub(super) fn line_bounds(&self) -> (i32, i32) {
        let state = self.shared.state();
        (
            -(state.engine.history_size() as i32),
            i32::from(state.size.rows) - 1,
        )
    }
    pub(super) fn visit_line_cells(
        &self,
        requested_first: i32,
        requested_last: i32,
        mut visitor: impl FnMut((i32, i32, usize), i32, usize, &TerminalRenderCell),
    ) -> (i32, i32, usize) {
        let state = self.shared.state();
        let range = (
            -(state.engine.history_size() as i32),
            i32::from(state.size.rows) - 1,
            usize::from(state.size.cols),
        );
        for line in requested_first.max(range.0)..=requested_last.min(range.1) {
            let wrapped = state.engine.line_wrapped(line);
            if let Some(cells) = state.engine.line(line) {
                for (col, cell) in cells.iter().enumerate() {
                    visitor(
                        range,
                        line,
                        col,
                        &render_cell(cell, wrapped && col + 1 == cells.len()),
                    );
                }
            }
        }
        range
    }
    pub(super) fn search(&self, query: &str) -> Vec<TermySearchMatch> {
        self.search_with_options(query, TermySearchOptions::default())
    }
    pub(super) fn search_with_options(
        &self,
        query: &str,
        options: TermySearchOptions,
    ) -> Vec<TermySearchMatch> {
        self.search_shared_with_options(query, options)
            .into_iter()
            .map(Into::into)
            .collect()
    }
    pub(super) fn search_shared(&self, query: &str) -> Vec<TermySharedSearchMatch> {
        self.search_shared_with_options(query, TermySearchOptions::default())
    }
    pub(super) fn search_shared_with_options(
        &self,
        query: &str,
        options: TermySearchOptions,
    ) -> Vec<TermySharedSearchMatch> {
        if query.is_empty() {
            return Vec::new();
        }
        let state = self.shared.state();
        let history = state.engine.history_size() as i32;
        search_lines_shared(
            (-history..i32::from(state.size.rows)).filter_map(|line| {
                state.engine.line(line).map(|cells| {
                    let mut text: String = cells.iter().map(search_character).collect();
                    text.truncate(text.trim_end().len());
                    ((line + history) as usize, text)
                })
            }),
            query,
            options,
        )
    }
    pub(super) fn hyperlink_at(&self, row: usize, col: usize) -> Option<DetectedLink> {
        crate::links::hyperlink_at_viewport_cell(&self.shared.state().engine, row, col)
    }

    pub(super) fn link_at(&self, row: usize, col: usize) -> Option<DetectedViewportLink> {
        crate::links::link_at_viewport_cell(&self.shared.state().engine, row, col)
    }

    pub(super) fn scroll_display(&self, delta: i32) -> bool {
        let mut state = self.shared.state();
        let changed = state.engine.scroll_display(delta);
        if changed {
            state.generation = state.generation.wrapping_add(1);
        }
        drop(state);
        if changed {
            self.shared.notify();
        }
        changed
    }
    pub(super) fn scroll_to_bottom(&self) -> bool {
        self.scroll_display(i32::MIN)
    }
    pub(super) fn clear_scrollback(&self) -> bool {
        let mut state = self.shared.state();
        let changed = state.engine.history_size() > 0;
        state.engine.clear_scrollback();
        state.generation = state.generation.wrapping_add(1);
        drop(state);
        if changed {
            self.shared.notify();
        }
        changed
    }
    pub(super) fn scroll_state(&self) -> (usize, usize) {
        let state = self.shared.state();
        (state.engine.display_offset(), state.engine.history_size())
    }
    pub(super) fn cursor_state(&self) -> Option<TerminalCursorState> {
        self.shared.state().cursor()
    }
    pub(super) fn cursor_position(&self) -> (usize, usize) {
        let cursor = self.shared.state().engine.cursor();
        (cursor.col, cursor.row)
    }
    pub(super) fn set_term_options(&self, options: TerminalOptions) {
        let mut state = self.shared.state();
        state.engine.set_options(engine::Options {
            scrollback_history: options
                .scrollback_history
                .min(MAX_TERMINAL_SCROLLBACK_HISTORY),
        });
        state
            .engine
            .set_default_cursor_shape(cursor_shape(options.default_cursor_style));
        state.default_cursor_style = options.default_cursor_style;
        state.generation = state.generation.wrapping_add(1);
        state.force_full_damage = true;
    }
    pub(super) fn set_scrollback_history(&self, history: usize) {
        let style = self.shared.state().default_cursor_style;
        self.set_term_options(TerminalOptions {
            scrollback_history: history,
            default_cursor_style: style,
        });
    }
    pub(super) fn bracketed_paste_mode(&self) -> bool {
        self.shared.state().engine.modes().bracketed_paste
    }
    pub(super) fn alternate_screen_mode(&self) -> bool {
        self.shared.state().engine.alternate_screen()
    }
    pub(super) fn keyboard_mode(&self) -> TerminalKeyboardMode {
        let modes = self.shared.state().engine.modes();
        let bits = modes.kitty_keyboard;
        TerminalKeyboardMode::from_flags(
            modes.application_cursor,
            bits & 1 != 0,
            bits & 2 != 0,
            bits & 4 != 0,
            bits & 8 != 0,
            bits & 16 != 0,
        )
    }
    pub(super) fn mouse_mode(&self) -> TerminalMouseMode {
        let modes = self.shared.state().engine.modes();
        TerminalMouseMode {
            enabled: modes.mouse_tracking != engine::MouseTracking::None,
            report_click: matches!(
                modes.mouse_tracking,
                engine::MouseTracking::Click | engine::MouseTracking::Press
            ),
            report_drag: modes.mouse_tracking == engine::MouseTracking::Drag,
            report_motion: modes.mouse_tracking == engine::MouseTracking::Motion,
            sgr_encoding: matches!(
                modes.mouse_encoding,
                engine::MouseEncoding::Sgr | engine::MouseEncoding::SgrPixels
            ),
            utf8_encoding: modes.mouse_encoding == engine::MouseEncoding::Utf8,
        }
    }
}

fn engine_size(size: TerminalSize) -> engine::Size {
    engine::Size {
        cols: usize::from(size.cols),
        rows: usize::from(size.rows),
    }
}
fn pty_size(size: TerminalSize) -> PtySize {
    PtySize {
        cols: size.cols,
        rows: size.rows,
        cell_width: size.cell_width,
        cell_height: size.cell_height,
    }
}
fn cursor_shape(style: TerminalCursorStyle) -> engine::CursorShape {
    match style {
        TerminalCursorStyle::Block => engine::CursorShape::Block,
        TerminalCursorStyle::Line => engine::CursorShape::Beam,
    }
}
fn rgb(color: engine::Color) -> Option<TerminalColor> {
    color.as_rgb().map(|(r, g, b)| TerminalColor { r, g, b })
}
fn render_color(color: engine::Color, foreground: bool) -> TerminalRenderColor {
    if let Some(rgb) = rgb(color) {
        TerminalRenderColor::Rgb(rgb)
    } else if let Some(index) = color.as_indexed() {
        TerminalRenderColor::Indexed(index)
    } else if foreground {
        TerminalRenderColor::DefaultForeground
    } else {
        TerminalRenderColor::DefaultBackground
    }
}
fn render_cell(cell: &engine::Cell, wrapped: bool) -> TerminalRenderCell {
    let style = cell.style;
    let attr = style.attributes;
    TerminalRenderCell {
        text: TerminalRenderText::from_cell_suffix(cell.character, Some(cell.combining())),
        foreground: render_color(style.foreground, true),
        background: render_color(style.background, false),
        underline_color: (style.underline_color != engine::Color::DEFAULT)
            .then(|| render_color(style.underline_color, true)),
        bold: attr & engine::Style::BOLD != 0,
        dim: attr & engine::Style::DIM != 0,
        italic: attr & engine::Style::ITALIC != 0,
        inverse: attr & engine::Style::INVERSE != 0,
        hidden: attr & engine::Style::HIDDEN != 0,
        strikethrough: attr & engine::Style::STRIKE != 0,
        underline_style: match style.underline {
            engine::UnderlineStyle::None => TerminalUnderlineStyle::None,
            engine::UnderlineStyle::Single => TerminalUnderlineStyle::Single,
            engine::UnderlineStyle::Double => TerminalUnderlineStyle::Double,
            engine::UnderlineStyle::Curly => TerminalUnderlineStyle::Curly,
            engine::UnderlineStyle::Dotted => TerminalUnderlineStyle::Dotted,
            engine::UnderlineStyle::Dashed => TerminalUnderlineStyle::Dashed,
        },
        hyperlink: cell.hyperlink().is_some(),
        wide_character_spacer: cell.flags & engine::Cell::WIDE_SPACER != 0,
        leading_wide_character_spacer: cell.flags & engine::Cell::LEADING_WIDE_SPACER != 0,
        line_wrapped: wrapped,
    }
}
fn resolve_color(
    color: TerminalRenderColor,
    palette: &TerminalPalette,
    fallback: TerminalQueryColors,
) -> TermyColor {
    let color = match color {
        TerminalRenderColor::Rgb(rgb) => rgb,
        TerminalRenderColor::Indexed(index) | TerminalRenderColor::DimIndexed(index) => {
            palette.indexed[index as usize].unwrap_or_else(|| fallback.indexed_color(index))
        }
        TerminalRenderColor::DefaultBackground => palette.background.unwrap_or(fallback.background),
        TerminalRenderColor::Cursor => palette
            .cursor
            .or(fallback.cursor)
            .unwrap_or(fallback.foreground),
        _ => palette.foreground.unwrap_or(fallback.foreground),
    };
    TermyColor {
        r: color.r,
        g: color.g,
        b: color.b,
        a: 255,
    }
}
fn legacy_cell(
    cell: &engine::Cell,
    wrapped: bool,
    palette: &TerminalPalette,
    fallback: TerminalQueryColors,
) -> TermyCell {
    let attributes = cell.style.attributes;
    let bold = attributes & engine::Style::BOLD != 0;
    let inverse = attributes & engine::Style::INVERSE != 0;
    let foreground = render_color(cell.style.foreground, true);
    let background = render_color(cell.style.background, false);
    let (mut foreground, background) = if inverse {
        (background, foreground)
    } else {
        (foreground, background)
    };
    if bold && let TerminalRenderColor::Indexed(index @ 0..=7) = foreground {
        foreground = TerminalRenderColor::Indexed(index + 8);
    }
    let mut fg = resolve_color(foreground, palette, fallback);
    if attributes & engine::Style::DIM != 0 {
        fg.r /= 2;
        fg.g /= 2;
        fg.b /= 2;
    }
    let spacer = cell.flags & (engine::Cell::WIDE_SPACER | engine::Cell::LEADING_WIDE_SPACER) != 0;
    TermyCell {
        char: cell.character,
        fg,
        bg: resolve_color(background, palette, fallback),
        uses_terminal_default_bg: background == TerminalRenderColor::DefaultBackground,
        bold,
        italic: attributes & engine::Style::ITALIC != 0,
        underline: cell.style.underline != engine::UnderlineStyle::None,
        strikethrough: attributes & engine::Style::STRIKE != 0,
        render_text: !spacer
            && attributes & engine::Style::HIDDEN == 0
            && cell.character != '\0'
            && !cell.character.is_control(),
        wide_character_spacer: spacer,
        line_wrapped: wrapped,
    }
}

fn search_character(cell: &engine::Cell) -> char {
    if cell.flags & (engine::Cell::WIDE_SPACER | engine::Cell::LEADING_WIDE_SPACER) != 0
        || cell.style.attributes & engine::Style::HIDDEN != 0
        || cell.character.is_control()
    {
        ' '
    } else {
        cell.character
    }
}
fn normalize_spans(spans: &mut Vec<TerminalDirtySpan>) {
    spans.sort_unstable_by_key(|span| (span.row, span.left_col));
    let mut len = 0;
    for index in 0..spans.len() {
        let span = spans[index];
        if len > 0
            && spans[len - 1].row == span.row
            && span.left_col <= spans[len - 1].right_col.saturating_add(1)
        {
            spans[len - 1].right_col = spans[len - 1].right_col.max(span.right_col);
        } else {
            spans[len] = span;
            len += 1;
        }
    }
    spans.truncate(len);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn display(cols: u16, rows: u16) -> CustomBackend {
        CustomBackend::new_display(
            TerminalSize {
                cols,
                rows,
                ..TerminalSize::default()
            },
            None,
        )
    }

    #[test]
    fn facade_preserves_wide_combining_and_history_cells() {
        let terminal = display(6, 2);
        terminal.feed_output("a界e\u{301}\r\nsecond\r\nlast".as_bytes());
        assert_eq!(terminal.scroll_state().1, 1);
        let mut observed = Vec::new();
        terminal.visit_line_cells(-1, -1, |_, _, _, cell| observed.push(cell.clone()));
        assert_eq!(observed[1].text.as_str(), "界");
        assert!(observed[2].wide_character_spacer);
        assert_eq!(observed[3].text.as_str(), "e\u{301}");
        assert_eq!(terminal.search("last")[0].row, 2);
    }

    #[test]
    fn generation_rejects_cells_changed_since_damage_read() {
        let terminal = display(6, 2);
        terminal.take_render_damage_snapshot();
        terminal.feed_output(b"x");
        let update = terminal.take_render_damage_snapshot();
        let TerminalDamageSnapshot::Partial(spans) = update.damage else {
            panic!("incremental damage");
        };
        let mut visited = 0;
        assert!(terminal.visit_viewport_ranges_at_generation(
            update.generation,
            &spans,
            |_, _, _, _, _| visited += 1
        ));
        assert!(visited > 0);
        terminal.feed_output(b"y");
        assert!(!terminal.visit_viewport_ranges_at_generation(
            update.generation,
            &spans,
            |_, _, _, _, _| panic!("stale read")
        ));
    }

    #[test]
    fn forced_read_consumes_full_damage_once() {
        let terminal = display(6, 2);
        assert!(matches!(
            terminal.render_read(true).update.damage,
            TerminalDamageSnapshot::Full
        ));
        assert_eq!(
            terminal.take_damage_snapshot(),
            TerminalDamageSnapshot::Partial(Vec::new())
        );
    }

    #[test]
    fn display_routes_cursor_and_clipboard_queries() {
        struct Host {
            replies: Vec<u8>,
        }
        impl TerminalReplyHost for Host {
            fn load_clipboard(&mut self, _: TerminalClipboardTarget) -> Option<String> {
                Some("hello".to_owned())
            }
            fn protocol_reply(&mut self, bytes: &[u8]) {
                self.replies.extend_from_slice(bytes);
            }
        }
        let terminal = display(6, 2);
        terminal.feed_output(b"hi\x1b[6n\x1b]52;c;?\x1b\\");
        let mut host = Host {
            replies: Vec::new(),
        };
        terminal.drain_events(&mut host);
        assert_eq!(host.replies, b"\x1b[1;3R\x1b]52;c;aGVsbG8=\x1b\\");
    }

    #[test]
    fn synchronized_output_commits_after_deadline_without_more_input() {
        let (send, receive) = flume::unbounded();
        let terminal = CustomBackend::new_display_with_wakeup_notifier(
            TerminalSize::default(),
            None,
            Some(TerminalWakeupNotifier::new(move || {
                let _ = send.send(());
            })),
        );
        terminal.render_read(true);
        terminal.feed_output(b"\x1b[?2026hdeadline");
        assert_eq!(terminal.snapshot().cells[0].char, ' ');
        receive
            .recv_timeout(Duration::from_secs(2))
            .expect("watchdog commits output");
        assert_eq!(terminal.snapshot().cells[0].char, 'd');
    }

    #[cfg(unix)]
    #[test]
    fn native_exit_commits_synchronized_tail_before_exit_event() {
        let terminal = CustomBackend::new_with_launch_and_wakeup_notifier(
            TerminalSize::default(),
            None,
            None,
            None,
            None,
            Some(&TerminalLaunch::Program {
                program: "/bin/sh".to_owned(),
                args: vec![
                    "-c".to_owned(),
                    "printf '\\033[?2026hfinal-tail'".to_owned(),
                ],
            }),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let (events, _) = terminal.drain_events(&mut |_| None);
            if events
                .iter()
                .any(|event| matches!(event, TerminalEvent::Exit))
            {
                let text: String = terminal
                    .snapshot()
                    .cells
                    .into_iter()
                    .map(|cell| cell.char)
                    .collect();
                assert!(text.starts_with("final-tail"));
                break;
            }
            assert!(Instant::now() < deadline, "child exited");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

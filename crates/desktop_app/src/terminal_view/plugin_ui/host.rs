use super::*;

impl PluginUiView {
    pub(in crate::terminal_view) fn belongs_to(&self, plugin_id: &str, revision: &str) -> bool {
        self.descriptor.plugin_id == plugin_id && self.revision == revision
    }
}

impl TerminalView {
    pub(in crate::terminal_view) fn open_plugin_ui(
        &mut self,
        plugin_id: &str,
        view_id: &str,
        revision: &str,
        target: PluginViewTarget,
        params: serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let (descriptor, current_revision) = self
            .plugin_runtime
            .view_with_revision(plugin_id, view_id)
            .ok_or_else(|| format!("Plugin view {plugin_id}.{view_id} is unavailable"))?;
        if current_revision != revision {
            return Err("Plugin changed before its view could open; try again".to_string());
        }
        let context = self.plugin_context(cx);
        match target {
            PluginViewTarget::Modal => self.close_command_palette(cx),
            PluginViewTarget::CommandPalette => {
                self.open_command_palette_in_mode(CommandPaletteMode::Commands, cx);
                self.command_palette_input_mut().clear();
            }
        }
        self.close_search(cx);
        self.cancel_rename_tab(cx);
        self.cancel_rename_workspace(cx);
        let _ = self.close_terminal_context_menu(cx);
        let _ = self.close_tab_context_menu(cx);
        let _ = self.close_new_tab_menu(cx);
        let parent = cx.entity().downgrade();
        let window_handle = window.window_handle();
        let runtime = self.plugin_runtime.clone();
        let revision = revision.to_string();
        let plugin_ui = cx.new(|cx| {
            PluginUiView::new(
                parent,
                window_handle,
                runtime,
                descriptor,
                revision,
                params,
                target,
                cx,
            )
        });
        self.plugin_ui = Some(plugin_ui.clone());
        plugin_ui.update(cx, |view, cx| {
            if target == PluginViewTarget::Modal {
                view.focus(window, cx);
            }
            view.load(context, cx);
        });
        if target == PluginViewTarget::CommandPalette {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
        self.notify_overlay(cx);
        Ok(())
    }

    pub(in crate::terminal_view) fn replace_plugin_ui(
        &mut self,
        plugin_id: &str,
        view_id: &str,
        revision: &str,
        params: serde_json::Value,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let plugin_ui = self
            .plugin_ui
            .as_ref()
            .cloned()
            .ok_or_else(|| "Plugin returned view.replace without an open view".to_string())?;
        if plugin_ui.read(cx).descriptor.plugin_id != plugin_id {
            return Err("Plugin cannot replace another plugin's view".to_string());
        }
        let (descriptor, current_revision) = self
            .plugin_runtime
            .view_with_revision(plugin_id, view_id)
            .ok_or_else(|| format!("Plugin view {plugin_id}.{view_id} is unavailable"))?;
        if current_revision != revision {
            return Err("Plugin changed before its view could be replaced".to_string());
        }
        let context = self.plugin_context(cx);
        plugin_ui.update(cx, |view, cx| {
            view.replace(descriptor, revision.to_string(), params, context, cx);
        });
        Ok(())
    }

    pub(in crate::terminal_view) fn command_palette_plugin_ui(
        &self,
        cx: &App,
    ) -> Option<gpui_kit::Entity<PluginUiView>> {
        self.plugin_ui
            .as_ref()
            .filter(|view| view.read(cx).target() == PluginViewTarget::CommandPalette)
            .cloned()
    }

    pub(in crate::terminal_view) fn modal_plugin_ui(
        &self,
        cx: &App,
    ) -> Option<gpui_kit::Entity<PluginUiView>> {
        self.plugin_ui
            .as_ref()
            .filter(|view| view.read(cx).target() == PluginViewTarget::Modal)
            .cloned()
    }

    pub(in crate::terminal_view) fn dismiss_command_palette_plugin_ui(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.command_palette_plugin_ui(cx).is_none() {
            return false;
        }
        self.plugin_ui = None;
        self.plugin_runtime.suspend_if_eventless();
        cx.notify();
        self.notify_overlay(cx);
        true
    }

    pub(in crate::terminal_view) fn close_plugin_ui(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.plugin_ui.take().is_none() {
            return;
        }
        self.plugin_runtime.suspend_if_eventless();
        self.focus_handle.focus(window, cx);
        cx.notify();
        self.notify_overlay(cx);
    }
}

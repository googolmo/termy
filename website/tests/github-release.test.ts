import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fetchGitHubReleases } from '../src/lib/github-release';

test('release listings refresh at runtime and share requests while prerendering', async () => {
  const originalFetch = globalThis.fetch;
  const originalPrerender = process.env.TSS_PRERENDERING;
  let calls = 0;
  let version = 'v1.0.0';
  let fail = false;
  globalThis.fetch = Object.assign(async () => {
    calls++;
    if (fail) return new Response('unavailable', { status: 503 });
    return Response.json([{
      id: 1, name: version, tag_name: version,
      published_at: '2026-10-02T00:00:00Z', prerelease: false,
      html_url: 'https://example.test/release', body: '',
      tarball_url: '', zipball_url: '', assets: [],
    }]);
  }, { preconnect: originalFetch.preconnect });
  try {
    delete process.env.TSS_PRERENDERING;
    assert.equal((await fetchGitHubReleases())[0].tagName, 'v1.0.0');
    version = 'v1.0.1';
    assert.equal((await fetchGitHubReleases())[0].tagName, 'v1.0.1');
    assert.equal(calls, 2);

    process.env.TSS_PRERENDERING = 'true';
    fail = true;
    await assert.rejects(fetchGitHubReleases(), /503/);
    fail = false;
    const [first, second] = await Promise.all([fetchGitHubReleases(), fetchGitHubReleases()]);
    assert.equal(calls, 4);
    assert.strictEqual(first, second);
    await fetchGitHubReleases();
    assert.equal(calls, 4);

    delete process.env.TSS_PRERENDERING;
    version = 'v1.0.2';
    assert.equal((await fetchGitHubReleases())[0].tagName, 'v1.0.2');
  } finally {
    globalThis.fetch = originalFetch;
    if (originalPrerender === undefined) delete process.env.TSS_PRERENDERING;
    else process.env.TSS_PRERENDERING = originalPrerender;
  }
});

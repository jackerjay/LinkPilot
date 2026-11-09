// Sample data only. Native Helper/profile delivery is verified separately.
(() => {
  const locale = new URLSearchParams(location.search).get('locale') || 'en-US';
  const language = { 'en-US': 'en', 'zh-Hans': 'zh-CN', 'zh-Hant': 'zh-TW', ja: 'ja-JP' }[locale];
  const labels = {
    'en-US': ['Work', 'Personal', 'Work websites', 'Personal websites', 'Choose for other links'],
    'zh-Hans': ['工作', '个人', '工作网站', '个人网站', '其他链接由我选择'],
    'zh-Hant': ['工作', '個人', '工作網站', '個人網站', '其他連結由我選擇'],
    ja: ['仕事用', '個人用', '仕事のサイト', '個人のサイト', '他のリンクは自分で選択'],
  }[locale];
  localStorage.setItem('linkpilot.theme', 'dark');
  const target = (browser, profile = null) => ({ browser, profile, workspace: null, incognito: false, new_window: false });
  let config = {
    version: 1, default_target: target('safari'), custom_browsers: [], workspaces: [],
    rules: [
      { id: 'work', enabled: true, source: 'gui', when: { op: 'url-host', pattern: 'work.example.com' }, then: { kind: 'open', target: target('chrome', 'Profile 1') }, note: labels[2], workspace_id: null },
      { id: 'personal', enabled: true, source: 'gui', when: { op: 'url-host', pattern: 'personal.example.com' }, then: { kind: 'open', target: target('brave', 'Default') }, note: labels[3], workspace_id: null },
      { id: 'ask', enabled: true, source: 'gui', when: { op: 'always' }, then: { kind: 'ask' }, note: labels[4], workspace_id: null },
    ],
    settings: { launch_at_login: false, record_query_strings: false, auto_check_updates: false, smart_routing_enabled: true, picker_style: 'frosted', ask_quick_open_modifier: 'command', profile_orders: {}, disabled_browsers: [], language, behavior_log_enabled: false, behavior_log_retention_days: 90 },
  };
  const browsers = [
    { id: 'chrome', display_name: 'Google Chrome', kind: 'chromium', executable: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', platform_app_id: 'com.google.Chrome', profile_root: null },
    { id: 'brave', display_name: 'Brave Browser', kind: 'chromium', executable: '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser', platform_app_id: 'com.brave.Browser', profile_root: null },
    { id: 'safari', display_name: 'Safari', kind: 'safari', executable: '/Applications/Safari.app/Contents/MacOS/Safari', platform_app_id: 'com.apple.Safari', profile_root: null },
  ];
  const profiles = { chrome: [
    { id: 'Default', display_name: labels[1], email: null, avatar_url: null, accent_color: '#60a5fa', is_default: true },
    { id: 'Profile 1', display_name: labels[0], email: null, avatar_url: null, accent_color: '#34d399', is_default: false },
  ], brave: [{ id: 'Default', display_name: labels[1], email: null, avatar_url: null, accent_color: '#fb923c', is_default: true }], safari: [] };
  const callbacks = new Map();
  const listeners = new Map();
  let next = 1;
  const emit = (event, payload) => {
    for (const [id, listener] of listeners) if (listener.event === event) callbacks.get(listener.handler)?.({ event, id, payload });
  };
  function explain(url) {
    const work = new URL(url).hostname === 'work.example.com';
    return work ? { decision: { action: 'open', target: target('chrome', 'Profile 1'), matched_rule: 'work', reason: 'matched-rule' }, explanation: { op: 'url-host', pattern: 'work.example.com', matched: true } }
      : { decision: { action: 'ask', candidates: [], matched_rule: 'ask', reason: 'matched-rule' }, explanation: { op: 'always', matched: true } };
  }
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: (_event, id) => listeners.delete(id) };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
    transformCallback: (callback) => { const id = next++; callbacks.set(id, callback); return id; },
    unregisterCallback: (id) => callbacks.delete(id),
    convertFileSrc: (path) => path,
    invoke: async (command, args = {}) => {
      switch (command) {
        case 'plugin:app|version': return '0.5.15';
        case 'plugin:app|name': return 'LinkPilot Store';
        case 'plugin:app|identifier': return 'app.linkpilot.store';
        case 'plugin:event|listen': {
          const id = next++; listeners.set(id, args);
          if (args.event === 'tray:navigate') {
            const scene = new URLSearchParams(parent.location.search).get('scene');
            const page = scene === '03-test-url' ? 'test-url' : scene === '04-profiles' ? 'browsers' : 'rules';
            queueMicrotask(() => { if (listeners.has(id)) callbacks.get(args.handler)?.({event:args.event,id,payload:page}); });
          }
          return id;
        }
        case 'plugin:event|unlisten': listeners.delete(args.eventId); return;
        case 'config_get': return structuredClone(config);
        case 'config_replace': config = structuredClone(args.doc); emit('config-changed', null); return;
        case 'doctor': return { daemon_version: '0.5.15', is_default_browser: true, config_path: '~/Library/Containers/app.linkpilot.store/Data/Library/Application Support/LinkPilot/linkpilot.config.json', installed_browser_count: 3, ipc_socket_path: null };
        case 'list_browsers': return browsers;
        case 'list_profiles': return profiles[args.browser] || [];
        case 'helper_status': return { connected: true, version: '0.5.15', profile_browsers: ['chrome', 'brave'], error: null };
        case 'app_icon': {
          const id = browsers.find((b) => b.platform_app_id === args.request?.bundle_id)?.id;
          return id ? { data_url: '/icons/' + id + '.png' } : null;
        }
        case 'route_history': return [];
        case 'suggestions_list': return [];
        case 'route_evaluate': return explain(args.request.url);
        case 'plugin:window|set_background_color': return;
        case 'plugin:window|is_maximized': return false;
        case 'plugin:window|is_fullscreen': return false;
        default: throw new Error(`Screenshot fixture does not support ${command}`);
      }
    },
  };
})();

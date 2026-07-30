/**
 * Mock implementation of window.__TAURI__ for browser-only testing
 * Loads BEFORE app.js and other feature modules
 * Provides deterministic fixtures for core IPC operations
 *
 * Usage: Include this script in index.html BEFORE loading app.js, or inject via Playwright page.addInitScript()
 */

(function() {
  // Prevent duplicate initialization
  if (window.__TAURI_MOCK_INITIALIZED__) {
    return;
  }
  window.__TAURI_MOCK_INITIALIZED__ = true;

  // Core invoke mock - returns fixtures based on cmd name
  window.__TAURI__ = window.__TAURI__ || {};
  window.__TAURI__.core = {
    invoke: async function(cmd, args) {
      const fixtures = {
        // ==========================================
        // Chat & API Endpoints (app::chat namespace)
        // ==========================================
        'app::chat::ask': async function(args) {
          // Mock chat response - generate contextual responses based on input
          const input = args?.input || '';

          // Simulate multi-turn conversation understanding
          if (input.includes('销售') || input.includes('给客户')) {
            return {
              type: 'text',
              content: `理解了。让我整理一下：
• **给谁**：销售团队 / 客户
• **场景**：落地页展示
• **状态**：初步构想阶段

是否需要澄清更多细节？`
            };
          } else if (input.includes('日语') && input.includes('英语')) {
            return {
              type: 'multi_plan_bundle',
              wave_id: 'wave-mkt-20260728',
              plans: [
                { id: 'plan-jp-001', name: '日语落地页', status: 'pending' },
                { id: 'plan-en-001', name: '英语落地页', status: 'pending' }
              ]
            };
          } else if (input.includes('登录') && input.includes('支付')) {
            return {
              type: 'clarify_continue',
              current_understanding: {
                what: '落地页项目',
                who: '销售/客户',
                constraints: ['暂不做登录', '暂不做支付']
              }
            };
          }

          // Default response
          return {
            type: 'text',
            content: `收到您的需求："${input}"。需要我帮您拆分成可执行的计划吗？`
          };
        },

        'app::chat::clarify': async function(args) {
          return { status: 'updated', draft: { clarifications: [] } };
        },

        // ========================
        // Split & Confirm (app::split namespace)
        // ========================
        'app::split::confirm': async function(args) {
          return {
            status: 'queued',
            job_id: `job-${Date.now()}`,
            wave_id: args?.wave_id || 'wave-test'
          };
        },

        'app::split::list': async function() {
          return [];
        },

        'app::split::get': async function(args) {
          return null;
        },

        // ========================
        // State Management (app::state namespace)
        // ========================
        'app::state::get_selected_path': async function() {
          return '/tmp/w1-6-test-project';
        },

        'app::state::get_session': async function() {
          return {
            id: 'test-session-' + Date.now(),
            draft: null,
            context: {}
          };
        },

        'app::state::set_selected_path': async function(args) {
          return { success: true };
        },

        'app::state::get_project_meta': async function() {
          return {
            name: 'W1-6 Test Project',
            path: '/tmp/w1-6-test-project',
            created_at: new Date().toISOString()
          };
        },

        // ========================
        // Plan & Run Operations (app::run namespace)
        // ========================
        'app::run::list': async function() {
          return [];
        },

        'app::run::get': async function(args) {
          return null;
        },

        'app::run::stop': async function(args) {
          return { success: true };
        },

        'app::run::resume': async function(args) {
          return { success: true };
        },

        'app::run::retry_task': async function(args) {
          return { success: true };
        },

        // ========================
        // Inspect & Result Operations
        // ========================
        'app::inspect::list': async function() {
          return [];
        },

        'app::inspect::get': async function(args) {
          return null;
        },

        'app::result::list': async function() {
          return [];
        },

        // ========================
        // Settings & Doctor
        // ========================
        'app::settings::get_all': async function() {
          return {
            theme: 'default',
            provider: 'anthropic',
            model: 'claude-3-opus-20240229'
          };
        },

        'app::doctor::summary': async function() {
          return {
            issues: [],
            health: 'healthy'
          };
        },

        'app::doctor::run_checks': async function() {
          return { passed: true, checks: [] };
        },

        // ========================
        // Wave & Batch Operations
        // ========================
        'app::wave::list': async function() {
          return [];
        },

        'app::wave::get': async function(args) {
          return null;
        },

        'app::wave::confirm': async function(args) {
          return {
            status: 'confirmed',
            wave_id: args?.wave_id || 'wave-test',
            plan_count: 2
          };
        },

        'app::wave::split_plan': async function(args) {
          return {
            status: 'splitted',
            plan_id: args?.plan_id,
            new_wave_id: 'wave-split-' + Date.now()
          };
        },

        // ========================
        // General utilities
        // ========================
        'app::version::get': async function() {
          return 'v1.0.0-dev';
        },

        'app::path::resolve': async function(args) {
          return args?.path || '.';
        },

        'app::file::read': async function(args) {
          return { content: '', error: null };
        },

        // ==========================================
        // Shell & Core (needed for navigation/boot)
        // ==========================================
        'get_projects': async function() {
          // Return a test project so app can navigate to chat
          return [
            {
              path: '/tmp/w1-6-test-project',
              name: 'W1-6 Test Project',
              exists: true,
              active_status: null,
              last_status: null,
              running_tasks: 0,
              total_tasks: null
            }
          ];
        },

        'get_settings_cmd': async function() {
          return {
            theme: 'default',
            provider: 'anthropic',
            model: 'claude-3-opus-20240229',
            max_parallel: 2,
            effort: 'high',
            default_provider: 'claude'
          };
        },

        'doctor_cmd': async function(args) {
          return {
            passed: true,
            checks: [],
            issues: []
          };
        },

        'meta': async function() {
          return {
            version: 'v1.0.0-test'
          };
        },

        // Chat operations
        'chat_list_sessions_cmd': async function(args) {
          return [
            {
              session_id: 'default',
              title: null,
              message_count: 0
            }
          ];
        },

        'chat_send_cmd': async function(args) {
          const input = args?.message || '';
          console.log('[MOCK] chat_send_cmd:', input);

          // Determine response based on input
          if (input.includes('日语') && input.includes('英语')) {
            return {
              type: 'multi_plan_bundle',
              session_id: args.sessionId || 'default',
              messages: [
                {
                  role: 'assistant',
                  content: `好的，理解本波要两件落地页任务：

**1. 日语落地页**
• 目标用户：日本市场潜在客群
• 核心内容：价值主张、功能展示、CTA
• 验收：可预览单页结构清晰

**2. 英语落地页**
• 目标用户：国际市场潜在客群
• 核心内容：与日语版本一致的框架
• 验收：多语言对照无误`
                }
              ],
              draft_plan: null
            };
          } else if (input.includes('销售') || input.includes('给客户') || input.includes('网页')) {
            return {
              session_id: args.sessionId || 'default',
              messages: [
                {
                  role: 'assistant',
                  content: '好的，理解了以下关键信息：\n\n给谁：销售团队 / 客户\n场景：落地页展示\n状态：初步构想阶段\n\n这是一个含糊需求，建议再澄清细节。是否要继续？'
                }
              ],
              draft_plan: null,
              fake: false
            };
          }

          // Default response
          return {
            session_id: args.sessionId || 'default',
            messages: [
              {
                role: 'assistant',
                content: `收到您的需求："${input}"。需要我帮您拆分成可执行的计划吗？`
              }
            ],
            draft_plan: null,
            fake: false
          };
        },

        'stream_partial_cmd': async function(args) {
          return { text: '', bytes: 0, done: false };
        },

        // Plan operations
        'get_plans': async function(args) {
          return [];
        },

        'get_project_live': async function(args) {
          return {
            running_tasks: 0,
            live_tasks: [],
            last_dismissed: null
          };
        },

        'latest_plan_job_cmd': async function(args) {
          return null;
        },

        'get_project_persona_cmd': async function(args) {
          return null;
        },

        'project_pins_list_cmd': async function(args) {
          return [];
        }
      };

      const handler = fixtures[cmd];
      if (handler) {
        try {
          return await handler(args);
        } catch (err) {
          throw new Error(`Mock handler error for ${cmd}: ${err.message}`);
        }
      }

      // Fallback for unmocked commands - log warning but don't fail
      console.warn(`[MOCK-TAURI] No mock registered for command: ${cmd}. Returning null.`);
      return null;
    }
  };

  // Event listeners mock (no-op for tests)
  window.__TAURI__.event = {
    listen: async function(event, callback) {
      // Return noop unsubscribe function
      return async function() {
        console.log(`[MOCK-EVENT] Unsubscribed from event: ${event}`);
      };
    },
    emit: async function(event, data) {
      console.log(`[MOCK-EVENT] Emitted: ${event}`, data);
      return true;
    }
  };

  // Dialog mock (no-op)
  window.__TAURI__.dialog = {
    open: async function(opts) {
      console.log('[MOCK-DIALOG] Open dialog called with:', opts);
      return null; // User "canceled"
    },
    save: async function(opts) {
      console.log('[MOCK-DIALOG] Save dialog called with:', opts);
      return null;
    },
    message: async function(opts) {
      console.log('[MOCK-DIALOG] Message dialog:', opts.message);
      return { button: 'ok' };
    }
  };

  // Plugins namespace mock (for compatibility)
  window.__TAURI__.plugins = {
    dialog: window.__TAURI__.dialog
  };

  console.log('[MOCK-TAURI] ✅ IPC layer initialized for testing');
  console.log('[MOCK-TAURI] 📦 Available handlers:', Object.keys(fixtures).length);
})();

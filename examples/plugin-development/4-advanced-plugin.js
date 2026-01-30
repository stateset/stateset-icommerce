export default function advancedPlugin(api, context) {
  const { config, manifest, runtime } = context;
  
  console.log(`[advanced-plugin] Loaded v${manifest.version}`);
  console.log(`[advanced-plugin] Config:`, JSON.stringify(config, null, 2));
  
  // ==========================================================================
  // Command with arguments
  // ==========================================================================
  
  api.registerCommand({
    name: 'complex_calc',
    description: 'Perform complex calculations with arithmetic operations',
    acceptsArgs: true,
    handler: async (args) => {
      const expression = args.trim();
      
      if (!expression) {
        return 'Usage: /complex_calc <expression>\nExample: /complex_calc "2 + 3 * 4"';
      }
      
      try {
        const result = eval(expression);
        return `Result: ${result}`;
      } catch (e) {
        return `Error evaluating expression: ${e.message}`;
      }
    },
  });
  
  // ==========================================================================
  // Hook Priority System
  // ==========================================================================
  
  // Priority 10: Runs early - adds contextual metadata
  api.on('message_sending', async (data) => {
    console.log(`[advanced-plugin] Early hook (priority 10): Adding metadata to message ${data.messageId}`);
    
    return {
      metadata: {
        processedBy: 'advanced-plugin',
        timestamp: new Date().toISOString(),
        pluginVersion: manifest.version,
      },
    };
  }, { priority: 10 });
  
  // Priority 100: Runs in middle - logs message details
  api.on('message_sending', async (data) => {
    console.log(`[advanced-plugin] Middle hook (priority 100): Message from ${data.message?.from}`);
    console.log(`  Channel: ${data.channelId}`);
    console.log(`  Content length: ${data.message?.content?.length || 0}`);
  }, { priority: 100 });
  
  // Priority 200: Runs late - final verification
  api.on('message_sending', async (data) => {
    if (config.requireMention && !data.metadata?.mentioned) {
      console.log(`[advanced-plugin] Late hook (priority 200): Message lacks mention, flagging for review`);
    }
  }, { priority: 200 });
  
  // ==========================================================================
  // Parallel Hook - Async Processing
  // ==========================================================================
  
  api.on('message_received', async (data) => {
    const content = data.message?.content || '';
    
    if (content.includes('urgent') || content.includes('asap')) {
      console.log(`[advanced-plugin] Urgent message detected for agent ${data.agentId}`);
      // Increment urgency counter in runtime state
      if (runtime && runtime.setState) {
        const currentState = runtime.getState() || {};
        runtime.setState({
          ...currentState,
          urgentMessageCount: (currentState.urgentMessageCount || 0) + 1,
        });
      }
    }
  });
  
  // ==========================================================================
  // Sequential Hook - Tool Call Interception
  // ==========================================================================
  
  api.on('before_tool_call', async (data) => {
    const toolName = data.toolName;
    
    // Log all tool calls
    console.log(`[advanced-plugin] Tool call intercepted: ${toolName}`);
    
    // Example: Block sensitive tools unless explicitly allowed
    const sensitiveTools = config.blockedTools || [];
    if (sensitiveTools.includes(toolName)) {
      console.log(`[advanced-plugin] Blocking sensitive tool: ${toolName}`);
      return { blocked: true, reason: 'Tool blocked by advanced-plugin' };
    }
    
    // Add execution context
    return {
      context: {
        pluginId: manifest.id,
        timestamp: new Date().toISOString(),
      },
    };
  }, { priority: 50 });
  
  // ==========================================================================
  // Service Dependencies
  // ==========================================================================
  
  let messageLog = [];
  let intervalId = null;
  
  api.registerService({
    name: 'message-logger',
    description: 'Logs messages to internal buffer with periodic flush',
    start: async () => {
      console.log('[advanced-plugin] Message logger service starting');
      
      // Start a periodic flush service
      if (config.flushInterval) {
        intervalId = setInterval(() => {
          flushLog();
        }, config.flushInterval * 1000);
      }
      
      // Hook into message lifecycle to collect data
      api.on('message_sent', async (data) => {
        messageLog.push({
          type: 'sent',
          messageId: data.messageId,
          timestamp: new Date().toISOString(),
        });
        
        if (messageLog.length >= (config.maxLogSize || 1000)) {
          flushLog();
        }
      });
      
      api.on('message_received', async (data) => {
        messageLog.push({
          type: 'received',
          agentId: data.agentId,
          timestamp: new Date().toISOString(),
        });
      });
    },
    stop: async () => {
      console.log('[advanced-plugin] Message logger service stopping');
      flushLog();
      
      if (intervalId) {
        clearInterval(intervalId);
        intervalId = null;
      }
    },
  });
  
  function flushLog() {
    if (messageLog.length === 0) return;
    
    console.log(`[advanced-plugin] Flushing ${messageLog.length} entries from log`);
    
    if (config.logFile && runtime && runtime.writeData) {
      runtime.writeData(config.logFile, JSON.stringify(messageLog, null, 2));
    }
    
    messageLog = [];
  }
  
  // ==========================================================================
  // HTTP Routes with REST API
  // ==========================================================================
  
  api.registerHttpRoute({
    method: 'GET',
    path: '/advanced/stats',
    handler: async (req, res) => {
      const state = runtime ? runtime.getState() : {};
      
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        plugin: manifest.id,
        version: manifest.version,
        urgentMessageCount: state.urgentMessageCount || 0,
        logEntries: messageLog.length,
        uptime: process.uptime(),
      }, null, 2));
    },
  });
  
  api.registerHttpRoute({
    method: 'POST',
    path: '/advanced/config',
    handler: async (req, res) => {
      let body = '';
      
      for await (const chunk of req) {
        body += chunk.toString();
      }
      
      try {
        const newConfig = JSON.parse(body);
        console.log('[advanced-plugin] Config update:', newConfig);
        
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ success: true, message: 'Config updated' }));
      } catch (e) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ success: false, error: e.message }));
      }
    },
  });
  
  api.registerHttpRoute({
    method: 'GET',
    path: '/advanced/health',
    handler: async (req, res) => {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        status: 'healthy',
        pluginId: manifest.id,
        version: manifest.version,
        services: ['message-logger'],
      }));
    },
  });
}
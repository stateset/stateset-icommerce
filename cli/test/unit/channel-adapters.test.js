/**
 * Unit tests for channel adapter logic patterns
 *
 * Each messaging channel defines an adapter with:
 * - extractText: Extract message text, strip mentions, filter own messages
 * - getSenderId: Return unique sender ID
 * - getTargetId: Return conversation/channel ID
 * - isOwnMessage: Detect bot's own messages
 *
 * These tests validate the adapter logic using mock platform message objects
 * matching each channel's native format.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';

// ============================================================================
// Slack Adapter Logic
// ============================================================================

describe('Slack adapter logic', () => {
  const BOT_USER_ID = 'U_BOT_123';

  function slackExtractText(event) {
    let text = event.text || '';
    if (event.channel_type !== 'im') {
      if (BOT_USER_ID && !text.includes(`<@${BOT_USER_ID}>`)) {
        if (!event.thread_ts) return null;
      }
      if (BOT_USER_ID) {
        text = text.replace(new RegExp(`<@${BOT_USER_ID}>`, 'g'), '').trim();
      }
    }
    return text || null;
  }

  const slackGetSenderId = (event) => event.user;
  const slackGetTargetId = (event) => event.channel;
  const slackIsOwnMessage = (event) => event.subtype === 'bot_message' || !!event.bot_id;

  it('extracts text from DM', () => {
    const event = {
      text: 'check order status',
      channel_type: 'im',
      user: 'U_USER',
      channel: 'D123',
    };
    assert.strictEqual(slackExtractText(event), 'check order status');
  });

  it('extracts text when mentioned in channel', () => {
    const event = {
      text: `<@${BOT_USER_ID}> list orders`,
      channel_type: 'channel',
      user: 'U_USER',
      channel: 'C123',
    };
    assert.strictEqual(slackExtractText(event), 'list orders');
  });

  it('returns null when not mentioned in channel (non-thread)', () => {
    const event = {
      text: 'hello everyone',
      channel_type: 'channel',
      user: 'U_USER',
      channel: 'C123',
    };
    assert.strictEqual(slackExtractText(event), null);
  });

  it('allows thread replies without mention', () => {
    const event = {
      text: 'follow up',
      channel_type: 'channel',
      user: 'U_USER',
      channel: 'C123',
      thread_ts: '1234.5678',
    };
    assert.strictEqual(slackExtractText(event), 'follow up');
  });

  it('gets sender ID from event.user', () => {
    assert.strictEqual(slackGetSenderId({ user: 'U_ALICE' }), 'U_ALICE');
  });

  it('gets target ID from event.channel', () => {
    assert.strictEqual(slackGetTargetId({ channel: 'C_GENERAL' }), 'C_GENERAL');
  });

  it('identifies bot messages by subtype', () => {
    assert.strictEqual(slackIsOwnMessage({ subtype: 'bot_message' }), true);
  });

  it('identifies bot messages by bot_id', () => {
    assert.strictEqual(slackIsOwnMessage({ bot_id: 'B123' }), true);
  });

  it('identifies user messages', () => {
    assert.strictEqual(slackIsOwnMessage({ user: 'U_ALICE' }), false);
  });
});

// ============================================================================
// Discord Adapter Logic
// ============================================================================

describe('Discord adapter logic', () => {
  const BOT_ID = '999888777';

  function discordExtractText(msg, mentionOnly = true) {
    let content = msg.content || '';
    if (mentionOnly && msg.guild) {
      const mentionRegex = new RegExp(`<@!?${BOT_ID}>`);
      if (!mentionRegex.test(content)) return null;
      content = content.replace(new RegExp(`<@!?${BOT_ID}>`, 'g'), '').trim();
    }
    return content || null;
  }

  const discordGetSenderId = (msg) => msg.author.id;
  const discordGetTargetId = (msg) => msg.channelId;
  const discordIsOwnMessage = (msg) => msg.author.id === BOT_ID || msg.author.bot;

  it('extracts text from DM (no guild)', () => {
    const msg = { content: 'hello bot', author: { id: 'U1', bot: false }, channelId: 'DM1' };
    assert.strictEqual(discordExtractText(msg), 'hello bot');
  });

  it('extracts text when mentioned in guild', () => {
    const msg = {
      content: `<@${BOT_ID}> show orders`,
      guild: true,
      author: { id: 'U1', bot: false },
      channelId: 'C1',
    };
    assert.strictEqual(discordExtractText(msg), 'show orders');
  });

  it('returns null when not mentioned in guild', () => {
    const msg = {
      content: 'random chat',
      guild: true,
      author: { id: 'U1', bot: false },
      channelId: 'C1',
    };
    assert.strictEqual(discordExtractText(msg), null);
  });

  it('strips nickname mention format <@!id>', () => {
    const msg = {
      content: `<@!${BOT_ID}> inventory`,
      guild: true,
      author: { id: 'U1', bot: false },
      channelId: 'C1',
    };
    assert.strictEqual(discordExtractText(msg), 'inventory');
  });

  it('gets sender ID', () => {
    assert.strictEqual(discordGetSenderId({ author: { id: 'U_BOB' } }), 'U_BOB');
  });

  it('gets target channel ID', () => {
    assert.strictEqual(discordGetTargetId({ channelId: 'C_GENERAL' }), 'C_GENERAL');
  });

  it('identifies own messages by bot ID', () => {
    assert.strictEqual(discordIsOwnMessage({ author: { id: BOT_ID, bot: true } }), true);
  });

  it('identifies other bot messages', () => {
    assert.strictEqual(discordIsOwnMessage({ author: { id: 'OTHER_BOT', bot: true } }), true);
  });

  it('identifies user messages', () => {
    assert.strictEqual(discordIsOwnMessage({ author: { id: 'U_HUMAN', bot: false } }), false);
  });
});

// ============================================================================
// Telegram Adapter Logic
// ============================================================================

describe('Telegram adapter logic', () => {
  const telegramExtractText = (ctx) => ctx.message?.text || null;
  const telegramGetSenderId = (ctx) => String(ctx.from.id);
  const telegramGetTargetId = (ctx) => ctx.chat.id;
  const telegramIsOwnMessage = () => false; // Bots don't receive own messages

  it('extracts text from message', () => {
    const ctx = { message: { text: 'list customers' }, from: { id: 12345 }, chat: { id: -67890 } };
    assert.strictEqual(telegramExtractText(ctx), 'list customers');
  });

  it('returns null for no message', () => {
    const ctx = { from: { id: 12345 }, chat: { id: -67890 } };
    assert.strictEqual(telegramExtractText(ctx), null);
  });

  it('returns null for non-text message', () => {
    const ctx = { message: { photo: {} }, from: { id: 12345 }, chat: { id: -67890 } };
    assert.strictEqual(telegramExtractText(ctx), null);
  });

  it('gets sender ID as string', () => {
    const ctx = { from: { id: 12345 }, chat: { id: -67890 } };
    assert.strictEqual(telegramGetSenderId(ctx), '12345');
  });

  it('gets target chat ID', () => {
    const ctx = { from: { id: 12345 }, chat: { id: -67890 } };
    assert.strictEqual(telegramGetTargetId(ctx), -67890);
  });

  it('never identifies own messages', () => {
    assert.strictEqual(telegramIsOwnMessage({}), false);
  });
});

// ============================================================================
// Teams Adapter Logic
// ============================================================================

describe('Teams adapter logic', () => {
  const APP_ID = 'teams-app-12345';

  function teamsExtractText(activity) {
    if (activity.type !== 'message') return null;
    let text = activity.text || '';
    // Strip bot mention (Teams includes <at>BotName</at>)
    text = text.replace(/<at>[^<]*<\/at>/gi, '').trim();
    return text || null;
  }

  function teamsGetSenderId(activity) {
    return activity.from?.aadObjectId || activity.from?.id || activity.from?.name || 'unknown';
  }

  function teamsGetTargetId(activity) {
    return activity.conversation?.id || '';
  }

  function teamsIsOwnMessage(activity) {
    return activity.from?.id === APP_ID || activity.from?.role === 'bot';
  }

  it('extracts text stripping at-mention', () => {
    const activity = {
      type: 'message',
      text: '<at>StateSet</at> show orders',
      from: { id: 'U1' },
      conversation: { id: 'C1' },
    };
    assert.strictEqual(teamsExtractText(activity), 'show orders');
  });

  it('returns null for non-message activities', () => {
    const activity = { type: 'conversationUpdate', from: { id: 'U1' } };
    assert.strictEqual(teamsExtractText(activity), null);
  });

  it('returns null for empty text after stripping', () => {
    const activity = {
      type: 'message',
      text: '<at>StateSet</at>',
      from: { id: 'U1' },
      conversation: { id: 'C1' },
    };
    assert.strictEqual(teamsExtractText(activity), null);
  });

  it('gets sender ID from AAD object ID', () => {
    assert.strictEqual(
      teamsGetSenderId({ from: { aadObjectId: 'AAD-123', id: 'fallback', name: 'Alice' } }),
      'AAD-123',
    );
  });

  it('falls back to id when no AAD', () => {
    assert.strictEqual(teamsGetSenderId({ from: { id: 'T-USER-1', name: 'Alice' } }), 'T-USER-1');
  });

  it('falls back to name', () => {
    assert.strictEqual(teamsGetSenderId({ from: { name: 'Alice' } }), 'Alice');
  });

  it('gets conversation ID', () => {
    assert.strictEqual(teamsGetTargetId({ conversation: { id: 'CONV-ABC' } }), 'CONV-ABC');
  });

  it('identifies own messages by app ID', () => {
    assert.strictEqual(teamsIsOwnMessage({ from: { id: APP_ID } }), true);
  });

  it('identifies own messages by bot role', () => {
    assert.strictEqual(teamsIsOwnMessage({ from: { id: 'other', role: 'bot' } }), true);
  });

  it('identifies user messages', () => {
    assert.strictEqual(teamsIsOwnMessage({ from: { id: 'U_HUMAN', role: 'user' } }), false);
  });
});

// ============================================================================
// Matrix Adapter Logic
// ============================================================================

describe('Matrix adapter logic', () => {
  const BOT_USER_ID = '@stateset:matrix.org';

  function matrixExtractText(raw) {
    if (raw.type !== 'm.room.message') return null;
    if (raw.content?.msgtype !== 'm.text') return null;
    return raw.content?.body || null;
  }

  const matrixGetSenderId = (raw) => raw.sender || '';
  const matrixGetTargetId = (raw) => raw.room_id || '';
  const matrixIsOwnMessage = (raw) => raw.sender === BOT_USER_ID;

  it('extracts text from m.room.message', () => {
    const raw = {
      type: 'm.room.message',
      content: { msgtype: 'm.text', body: 'hello' },
      sender: '@alice:matrix.org',
      room_id: '!room1',
    };
    assert.strictEqual(matrixExtractText(raw), 'hello');
  });

  it('returns null for non-message events', () => {
    const raw = { type: 'm.room.member', sender: '@alice:matrix.org', room_id: '!room1' };
    assert.strictEqual(matrixExtractText(raw), null);
  });

  it('returns null for non-text messages (images)', () => {
    const raw = {
      type: 'm.room.message',
      content: { msgtype: 'm.image', body: 'photo.jpg' },
      sender: '@alice:matrix.org',
      room_id: '!room1',
    };
    assert.strictEqual(matrixExtractText(raw), null);
  });

  it('gets sender from event', () => {
    assert.strictEqual(matrixGetSenderId({ sender: '@bob:matrix.org' }), '@bob:matrix.org');
  });

  it('gets room ID', () => {
    assert.strictEqual(matrixGetTargetId({ room_id: '!abc:matrix.org' }), '!abc:matrix.org');
  });

  it('identifies own messages', () => {
    assert.strictEqual(matrixIsOwnMessage({ sender: BOT_USER_ID }), true);
  });

  it('identifies user messages', () => {
    assert.strictEqual(matrixIsOwnMessage({ sender: '@alice:matrix.org' }), false);
  });
});

// ============================================================================
// WhatsApp Adapter Logic
// ============================================================================

describe('WhatsApp adapter logic', () => {
  const whatsappExtractText = (msg) => msg.body || null;
  const whatsappGetSenderId = (msg) => msg.from;
  const whatsappGetTargetId = (msg) => msg.from; // reply to sender
  const whatsappIsOwnMessage = (msg) => msg.fromMe;

  it('extracts text from message body', () => {
    assert.strictEqual(whatsappExtractText({ body: 'track my order' }), 'track my order');
  });

  it('returns null for empty body', () => {
    assert.strictEqual(whatsappExtractText({ body: '' }), null);
  });

  it('gets sender from .from', () => {
    assert.strictEqual(whatsappGetSenderId({ from: '+1234567890@c.us' }), '+1234567890@c.us');
  });

  it('identifies own messages', () => {
    assert.strictEqual(whatsappIsOwnMessage({ fromMe: true }), true);
  });

  it('identifies incoming messages', () => {
    assert.strictEqual(whatsappIsOwnMessage({ fromMe: false }), false);
  });
});

// ============================================================================
// Signal Adapter Logic
// ============================================================================

describe('Signal adapter logic', () => {
  const signalExtractText = (msg) => msg.body || msg.message || null;
  const signalGetSenderId = (msg) => msg.source || msg.sourceNumber || '';
  const signalGetTargetId = (msg) => msg.source || msg.groupId || '';

  it('extracts text from body', () => {
    assert.strictEqual(signalExtractText({ body: 'hello' }), 'hello');
  });

  it('extracts text from message field', () => {
    assert.strictEqual(signalExtractText({ message: 'hello' }), 'hello');
  });

  it('returns null when no text', () => {
    assert.strictEqual(signalExtractText({}), null);
  });

  it('gets sender from source', () => {
    assert.strictEqual(signalGetSenderId({ source: '+15551234567' }), '+15551234567');
  });

  it('gets target from source (DM)', () => {
    assert.strictEqual(signalGetTargetId({ source: '+15551234567' }), '+15551234567');
  });

  it('gets target from groupId', () => {
    assert.strictEqual(signalGetTargetId({ groupId: 'GRP_ABC' }), 'GRP_ABC');
  });
});

// ============================================================================
// Google Chat Adapter Logic
// ============================================================================

describe('Google Chat adapter logic', () => {
  const gchatExtractText = (event) => {
    if (event.type !== 'MESSAGE') return null;
    return event.message?.argumentText?.trim() || event.message?.text?.trim() || null;
  };

  const gchatGetSenderId = (event) => event.user?.name || '';
  const gchatGetTargetId = (event) => event.space?.name || '';

  it('extracts argumentText (mention stripped)', () => {
    const event = {
      type: 'MESSAGE',
      message: { argumentText: ' show orders ', text: '@Bot show orders' },
      user: { name: 'users/123' },
      space: { name: 'spaces/abc' },
    };
    assert.strictEqual(gchatExtractText(event), 'show orders');
  });

  it('falls back to text when no argumentText', () => {
    const event = {
      type: 'MESSAGE',
      message: { text: 'direct message' },
      user: { name: 'users/123' },
      space: { name: 'spaces/abc' },
    };
    assert.strictEqual(gchatExtractText(event), 'direct message');
  });

  it('returns null for non-MESSAGE events', () => {
    const event = {
      type: 'ADDED_TO_SPACE',
      user: { name: 'users/123' },
      space: { name: 'spaces/abc' },
    };
    assert.strictEqual(gchatExtractText(event), null);
  });

  it('gets sender from user.name', () => {
    assert.strictEqual(gchatGetSenderId({ user: { name: 'users/123456' } }), 'users/123456');
  });

  it('gets space from space.name', () => {
    assert.strictEqual(gchatGetTargetId({ space: { name: 'spaces/AAAA' } }), 'spaces/AAAA');
  });
});

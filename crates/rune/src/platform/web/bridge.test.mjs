// The password sink's input classes: `node --test crates/rune/src/platform/web/bridge.test.mjs`.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { passwordInput, secureKind } from './bridge.js';

// A password input holding `value`, matching `pseudo` (`:autofill` or `:-webkit-autofill`) if
// given; like a browser, it throws on the other pseudo-class.
function sink(value, pseudo) {
    return { value, matches: s => { if (pseudo && s !== pseudo) throw new SyntaxError(s); return s === pseudo; } };
}

// What `passwordInput` sends for `event` on an input holding `value`, and what it leaves there.
function run(event, value, { key = 'pw', pseudo } = {}) {
    const input = sink(value, pseudo), sent = [];
    passwordInput(event, input, key, m => sent.push(m));
    return { sent, left: input.value };
}

test('a keystroke types at the caret', () => {
    for (const key of ['a', 'é', '👍🏽', '🇦🇺', '👩‍💻']) assert.equal(secureKind('insertText', key, false, key), 'secure', key);
});

test('several characters inserted at once fill the field', () => {
    // Playwright's fill() and execCommand('insertText') select the field, then insert the value.
    for (const value of ['ab', '0123456789abc', 'sëcret']) assert.equal(secureKind('insertText', value, false, value), 'secure_replace', value);
    // A script that sets the value and sends an insertText with no data.
    assert.equal(secureKind('insertText', null, false, '0123456789abc'), 'secure_replace');
    assert.equal(secureKind('insertText', null, false, 'a'), 'secure');
});

test('a replacement, an input with no type and an autofilled field fill it', () => {
    assert.equal(secureKind('insertReplacementText', null, false, 'saved-password'), 'secure_replace');
    assert.equal(secureKind(undefined, undefined, false, 'saved-password'), 'secure_replace');
    assert.equal(secureKind('', null, false, 'saved-password'), 'secure_replace');
    assert.equal(secureKind('insertText', 'x', true, 'x'), 'secure_replace');
});

test('an empty fill clears the field, and empty typing sends nothing', () => {
    assert.equal(secureKind(undefined, undefined, false, ''), 'secure_replace');
    assert.equal(secureKind('insertReplacementText', null, false, ''), 'secure_replace');
    assert.equal(secureKind('insertText', '', false, ''), null);
    assert.equal(secureKind('deleteContentBackward', null, false, ''), null);
});

test('a paste or a drop that reaches the sink types at the caret, never replacing the field', () => {
    assert.equal(secureKind('insertFromPaste', null, false, 'pasted'), 'secure');
    assert.equal(secureKind('insertFromDrop', null, false, 'dropped'), 'secure');
});

test('the listener sends a keystroke or a fill and empties the input after every event', () => {
    assert.deepEqual(run({ inputType: 'insertText', data: 'a' }, 'a'),
        { sent: [{ kind: 'secure', key: 'pw', text: 'a' }], left: '' });
    assert.deepEqual(run({ inputType: 'insertText', data: null }, '0123456789abc'),
        { sent: [{ kind: 'secure_replace', key: 'pw', text: '0123456789abc' }], left: '' });
    assert.deepEqual(run({}, 'saved'), { sent: [{ kind: 'secure_replace', key: 'pw', text: 'saved' }], left: '' });
    for (const pseudo of [':autofill', ':-webkit-autofill']) {
        assert.deepEqual(run({ inputType: 'insertText', data: 'x' }, 'x', { pseudo }),
            { sent: [{ kind: 'secure_replace', key: 'pw', text: 'x' }], left: '' }, pseudo);
    }
});

test('the listener sends an empty fill and drops empty typing', () => {
    assert.deepEqual(run({}, ''), { sent: [{ kind: 'secure_replace', key: 'pw', text: '' }], left: '' });
    assert.deepEqual(run({ inputType: 'insertText', data: '' }, ''), { sent: [], left: '' });
});

test('the listener sends nothing while no Rune field has the keyboard, and still empties the input', () => {
    assert.deepEqual(run({}, 'saved', { key: '' }), { sent: [], left: '' });
    assert.deepEqual(run({ inputType: 'insertText', data: 'a' }, 'a', { key: '' }), { sent: [], left: '' });
});

test('without Intl.Segmenter an insertText types, and only the other signals fill', async () => {
    const segmenter = Object.getOwnPropertyDescriptor(Intl, 'Segmenter');
    delete Intl.Segmenter;
    try {
        const bare = await import('./bridge.js?no-segmenter');
        for (const value of ['👍🏽', '0123456789abc']) {
            assert.equal(bare.secureKind('insertText', value, false, value), 'secure', value);
            assert.equal(bare.secureKind('insertText', null, false, value), 'secure', value);
        }
        assert.equal(bare.secureKind(undefined, undefined, false, 'saved'), 'secure_replace');
        assert.equal(bare.secureKind('insertReplacementText', null, false, 'saved'), 'secure_replace');
        assert.equal(bare.secureKind('insertText', 'x', true, 'x'), 'secure_replace');
    } finally {
        Object.defineProperty(Intl, 'Segmenter', segmenter);
    }
});

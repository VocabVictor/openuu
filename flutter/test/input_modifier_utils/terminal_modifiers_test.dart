import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/input_modifier_utils.dart';

void main() {
  group('shouldApplyTerminalInputModifiers', () {
    test('accepts ordinary single-character keyboard input', () {
      expect(shouldApplyTerminalInputModifiers('a'), isTrue);
      expect(shouldApplyTerminalInputModifiers(' '), isTrue);
      expect(shouldApplyTerminalInputModifiers('/'), isTrue);
    });

    test('accepts supplementary-plane single-character keyboard input', () {
      expect(shouldApplyTerminalInputModifiers('😀'), isTrue);
    });

    test('rejects terminal control bytes and multi-character sequences', () {
      for (final input in ['\x00', '\x03', '\t', '\n', '\r', '\x1B', '\x7F']) {
        expect(
          shouldApplyTerminalInputModifiers(input),
          isFalse,
          reason: '${input.codeUnits} must not consume a one-shot modifier',
        );
      }
      expect(shouldApplyTerminalInputModifiers('\x1B[A'), isFalse);
    });
  });

  group('applyTerminalInputModifiers', () {
    test('keeps decomposed graphemes intact under Ctrl', () {
      const decomposedEAcute = 'e\u0301';

      expect(
        applyTerminalInputModifiers(
          decomposedEAcute,
          ctrlLocked: true,
          altLocked: false,
        ),
        decomposedEAcute,
      );
    });

    test('keeps non-ASCII graphemes intact under Ctrl', () {
      for (final input in ['é', '😀']) {
        expect(
          applyTerminalInputModifiers(
            input,
            ctrlLocked: true,
            altLocked: false,
          ),
          input,
        );
      }
    });

    test('maps Ctrl underscore to unit separator', () {
      expect(
        applyTerminalInputModifiers(
          '_',
          ctrlLocked: true,
          altLocked: false,
        ),
        '\x1F',
      );
    });

    test('maps the complete Ctrl symbol range', () {
      const mappings = {
        '[': '\x1B',
        r'\': '\x1C',
        ']': '\x1D',
        '^': '\x1E',
        '_': '\x1F',
        '/': '\x1F',
      };

      for (final entry in mappings.entries) {
        expect(
          applyTerminalInputModifiers(
            entry.key,
            ctrlLocked: true,
            altLocked: false,
          ),
          entry.value,
          reason: 'Ctrl+${entry.key} should map to ${entry.value.codeUnits}',
        );
      }
    });

    test('applies Ctrl before Alt for combined modifiers', () {
      expect(
        applyTerminalInputModifiers(
          'b',
          ctrlLocked: true,
          altLocked: true,
        ),
        '\x1B\x02',
      );
    });
  });

  group('terminalPastePayload', () {
    test('wraps paste text when bracketed paste mode is active', () {
      expect(
        terminalPastePayload('d', bracketedPasteMode: true),
        '\x1B[200~d\x1B[201~',
      );
    });

    test('keeps a lone newline unchanged when bracketed paste is disabled', () {
      expect(
        terminalPastePayload('\n', bracketedPasteMode: false),
        '\n',
      );
    });
  });
}

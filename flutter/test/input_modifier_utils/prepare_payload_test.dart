import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/input_modifier_utils.dart';

void main() {
  group('prepareTerminalInputPayload', () {
    test('normalizes a mobile keyboard Enter to carriage return', () {
      expect(
        prepareTerminalInputPayload(
          '\n',
          source: TerminalInputSource.keyboard,
          isMobileOrWebMobile: true,
          bracketedPasteMode: false,
          ctrlLocked: false,
          altLocked: false,
        ),
        '\r',
      );
    });

    test('keeps Ctrl+J as line feed on mobile', () {
      expect(
        prepareTerminalInputPayload(
          'j',
          source: TerminalInputSource.keyboard,
          isMobileOrWebMobile: true,
          bracketedPasteMode: false,
          ctrlLocked: true,
          altLocked: false,
        ),
        '\n',
      );
    });

    test('does not apply Alt to a terminal control byte', () {
      expect(
        prepareTerminalInputPayload(
          '\x1B',
          source: TerminalInputSource.keyboard,
          isMobileOrWebMobile: true,
          bracketedPasteMode: false,
          ctrlLocked: false,
          altLocked: true,
        ),
        '\x1B',
      );
    });

    test('keeps large keyboard payloads unchanged when modifiers are inactive',
        () {
      final payload = 'd' * (1024 * 1024);

      expect(
        prepareTerminalInputPayload(
          payload,
          source: TerminalInputSource.keyboard,
          isMobileOrWebMobile: false,
          bracketedPasteMode: false,
          ctrlLocked: false,
          altLocked: false,
        ),
        payload,
      );
    });

    test('keeps decomposed graphemes intact with locked keyboard modifiers',
        () {
      const decomposedEAcute = 'e\u0301';

      expect(
        prepareTerminalInputPayload(
          decomposedEAcute,
          source: TerminalInputSource.keyboard,
          isMobileOrWebMobile: true,
          bracketedPasteMode: false,
          ctrlLocked: true,
          altLocked: false,
        ),
        decomposedEAcute,
      );
    });

    test('preserves a lone pasted newline when modifiers are locked', () {
      expect(
        prepareTerminalInputPayload(
          '\n',
          source: TerminalInputSource.paste,
          isMobileOrWebMobile: true,
          bracketedPasteMode: false,
          ctrlLocked: true,
          altLocked: true,
        ),
        '\n',
      );
    });

    test('wraps paste without applying locked modifiers', () {
      expect(
        prepareTerminalInputPayload(
          'd',
          source: TerminalInputSource.paste,
          isMobileOrWebMobile: true,
          bracketedPasteMode: true,
          ctrlLocked: true,
          altLocked: true,
        ),
        '\x1B[200~d\x1B[201~',
      );
    });
  });
}

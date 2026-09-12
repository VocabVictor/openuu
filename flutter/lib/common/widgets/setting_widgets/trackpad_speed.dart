part of 'setting_widgets.dart';

class TrackpadSpeedWidget extends StatefulWidget {
  final SimpleWrapper<int> value;
  // If null, no debouncer will be applied.
  final Function(int)? onDebouncer;
  final ValueChanged<String>? onTextChanged;
  // IME actions call TextField.onSubmitted without reaching the dialog's
  // raw Enter handler, so the dialog needs a separate submission callback.
  final ValueChanged<String>? onTextSubmitted;

  TrackpadSpeedWidget({
    Key? key,
    required this.value,
    this.onDebouncer,
    this.onTextChanged,
    this.onTextSubmitted,
  });

  @override
  TrackpadSpeedWidgetState createState() => TrackpadSpeedWidgetState();
}

class TrackpadSpeedWidgetState extends State<TrackpadSpeedWidget> {
  final TextEditingController _controller = TextEditingController();
  late final Debouncer<int> debouncerSpeed;

  set value(int v) => widget.value.value = v;
  int get value => widget.value.value;

  void updateValue(int newValue) {
    setState(() {
      value = newValue.clamp(kMinTrackpadSpeed, kMaxTrackpadSpeed);
      // Scale the trackpad speed value to a percentage for display purposes.
      _controller.text = value.toString();
      if (widget.onDebouncer != null) {
        debouncerSpeed.setValue(value);
      }
    });
    widget.onTextChanged?.call(_controller.text);
  }

  void updateTextValue(String text) {
    widget.onTextChanged?.call(text);
    final newValue = int.tryParse(text);
    if (newValue == null ||
        newValue < kMinTrackpadSpeed ||
        newValue > kMaxTrackpadSpeed) {
      return;
    }
    setState(() => value = newValue);
  }

  void submitTextValue(String text) {
    final onTextSubmitted = widget.onTextSubmitted;
    if (onTextSubmitted != null) {
      onTextSubmitted(text);
      return;
    }
    if (widget.onTextChanged != null) {
      return;
    }
    final newValue = int.tryParse(text);
    if (newValue == null) {
      return;
    }
    updateValue(newValue);
  }

  @override
  void initState() {
    super.initState();
    debouncerSpeed = Debouncer<int>(
      Duration(milliseconds: 1000),
      onChanged: widget.onDebouncer,
      initialValue: widget.value.value,
    );
  }

  @override
  Widget build(BuildContext context) {
    if (_controller.text.isEmpty) {
      _controller.text = value.toString();
    }
    return Row(
      children: [
        Expanded(
          flex: 3,
          child: Slider(
            value: value.toDouble(),
            min: kMinTrackpadSpeed.toDouble(),
            max: kMaxTrackpadSpeed.toDouble(),
            divisions: ((kMaxTrackpadSpeed - kMinTrackpadSpeed) / 10).round(),
            onChanged: (double v) => updateValue(v.round()),
          ),
        ),
        Expanded(
            flex: 1,
            child: Row(
              children: [
                SizedBox(
                  width: 56,
                  child: TextField(
                    controller: _controller,
                    keyboardType: TextInputType.number,
                    textAlign: TextAlign.center,
                    onChanged: updateTextValue,
                    onSubmitted: submitTextValue,
                    style: const TextStyle(fontSize: 13),
                    decoration: InputDecoration(
                      contentPadding:
                          EdgeInsets.symmetric(vertical: 8.0, horizontal: 12.0),
                    ),
                  ),
                ).marginOnly(right: 8.0),
                Text(
                  '%',
                  style: const TextStyle(fontSize: 15),
                )
              ],
            )),
      ],
    );
  }
}

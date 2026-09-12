# Remote wake

The desktop main window registers a heartbeat with the configured account API every 10 seconds while the app is running (including when minimized to the tray). A target must have signed into this server at least once. After 30 seconds without a heartbeat it is considered offline for wake purposes; this is not proof of physical power state.

A helper must be running OpenUU under the same account, have a live account session, and have discovered the target on its LAN previously. Discovery stores the target MAC locally. The server forwards only the target ID, and the helper uses the existing `mainWol` implementation and local LAN cache to send magic packets. IP addresses alone are not used to infer LAN membership. Cached discovery cannot prove that a machine has not moved networks.

In More tools, Wake up is hidden while the target heartbeat is online. Otherwise it is enabled only when the server reports a suitable helper. Requests expire after 30 seconds and are consumed once; duplicate pending requests are rejected. Delivery failure requires a manual retry. A queued request is not an acknowledgement that a packet was sent or that the machine booted.

Enable BIOS/UEFI WOL and NIC magic-packet wake on the target, prefer wired Ethernet, and keep standby power connected. First sign in on both PCs and discover the target from the helper while both are online. Leave the helper app running, shut down/sleep the target, wait at least 30 seconds, then request wake from another signed-in client. Verify that the target actually boots and reconnects. The app does not change BIOS or Windows NIC settings automatically.

This version needs the desktop app running on the helper; the headless Windows service does not poll wake jobs. Restart and shutdown remain unsupported in the device menu. The server and client changes must both be deployed before this works. Hardware wake has not been verified by automated tests.

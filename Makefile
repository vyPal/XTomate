build-xt:
	@cargo build --release

build-notify-send-plugin:
	@cd plugins/notify-send && cargo build --release
	@cp plugins/notify-send/target/release/libnotify_send.so plugins/libnotify_send.so

run: build-xt build-notify-send-plugin
	@./target/release/xt run example

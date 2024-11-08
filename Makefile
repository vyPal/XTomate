build-xt:
	@cargo build --release

build-notify-send-plugin:
	@cd plugins/notify-send && cargo build --release
	@cp plugins/notify-send/target/release/libnotify_send.so plugins/libnotify_send.so

build-logger-plugin:
	@cd plugins/logger && cargo build --release
	@cp plugins/logger/target/release/liblogger.so plugins/liblogger.so

run: build-xt build-notify-send-plugin build-logger-plugin
	@./target/release/xt run example

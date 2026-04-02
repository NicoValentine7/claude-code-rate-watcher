.PHONY: run build clean swift-run swift-build swift-test app package

run:
	cargo run

build:
	cargo build --release

clean:
	cargo clean

swift-run:
	swift run ccrw

swift-build:
	swift build -c release

swift-test:
	swift test

app:
	./scripts/build_app.sh release

package:
	./scripts/package_release.sh

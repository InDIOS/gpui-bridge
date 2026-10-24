package gpui_bridge

import (
	"os"
	"path/filepath"
	"runtime"
)

// defaultHostPath resolves the host binary in the same order as the other clients:
//
//  1. $GPUI_BRIDGE_HOST, if set.
//  2. bin/ next to this package (where stage-host.mjs or the published package puts it).
//  3. A Cargo build, walking up from this package (target/debug or target/release).
//
// None of these needs to know how many folders separate the client from the repository
// root, so the lookup survives the client moving inside packages/.
func defaultHostPath() string {
	if env := os.Getenv("GPUI_BRIDGE_HOST"); env != "" {
		return env
	}
	exe := "gpui-bridge-host"
	if runtime.GOOS == "windows" {
		exe += ".exe"
	}

	// This package's directory (gpui_bridge/), whatever the cwd.
	_, file, _, _ := runtime.Caller(0)
	pkgDir := filepath.Dir(file)

	if bin := filepath.Join(pkgDir, "bin", exe); exists(bin) {
		return bin
	}

	dir := pkgDir
	for i := 0; i < 8; i++ {
		dir = filepath.Dir(dir)
		for _, profile := range []string{"debug", "release"} {
			cand := filepath.Join(dir, "target", profile, exe)
			if exists(cand) {
				return cand
			}
		}
		if dir == filepath.Dir(dir) {
			break // reached the filesystem root
		}
	}
	return filepath.Join(pkgDir, "..", "..", "..", "target", "debug", exe) // fallback
}

func exists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

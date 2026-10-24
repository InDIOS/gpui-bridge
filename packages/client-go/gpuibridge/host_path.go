package gpuibridge

import (
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
)

// hostDownloadHint tells a user of the published module where the host comes from: Go modules
// are source only, so the binary is not part of the download (docs/RELEASING.md).
const hostDownloadHint = "download gpui-bridge-host for your platform from " +
	"https://github.com/InDIOS/gpui-bridge/releases and put it next to your program, " +
	"on PATH, or in $GPUI_BRIDGE_HOST"

// defaultHostPath resolves the host binary:
//
//  1. $GPUI_BRIDGE_HOST, if set.
//  2. Next to the running executable (how a Go program ships the host).
//  3. On PATH.
//  4. A Cargo build, walking up from this package (target/debug or target/release): a checkout.
//
// The walk does not need to know how many folders separate the client from the repository
// root, so the lookup survives the client moving inside packages/.
func defaultHostPath() string {
	if env := os.Getenv("GPUI_BRIDGE_HOST"); env != "" {
		return env
	}
	exe := "gpui-bridge-host"
	if runtime.GOOS == "windows" {
		exe += ".exe"
	}

	if self, err := os.Executable(); err == nil {
		if beside := filepath.Join(filepath.Dir(self), exe); exists(beside) {
			return beside
		}
	}
	if onPath, err := exec.LookPath(exe); err == nil {
		return onPath
	}

	// This package's source directory, whatever the cwd.
	_, file, _, _ := runtime.Caller(0)
	pkgDir := filepath.Dir(file)
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

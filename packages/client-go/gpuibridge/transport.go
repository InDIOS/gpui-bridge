package gpuibridge

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"strings"
	"sync"
	"time"
)

// HostEvents reports what happens while the host runs.
type HostEvents struct {
	// OnMessage gets every parsed host → client message (key `type`).
	OnMessage func(message map[string]any)
	// OnCorruptLine gets a stdout line that is not a protocol message.
	OnCorruptLine func(line string)
	// OnExit is called when the host exits.
	OnExit func(exit HostExit)
}

// HostExit describes how the host exited.
type HostExit struct {
	// The process exit code, if it could be read.
	Code *int
	// The signal name, if a signal ended it.
	Signal string
	// The last lines the host wrote to stderr, for crash reports.
	StderrTail []string
	// The spawn error, if the process could not even start.
	SpawnError error
}

// HostError is an `error` message from the host: the operation was rejected and the host keeps running.
type HostError struct {
	Code    string
	Message string
	Op      string
}

const stderrTailLines = 20

// Transport owns a running host process and its JSON Lines stdin/stdout streams.
type Transport struct {
	proc   *os.Process
	state  *os.ProcessState
	stdin  io.WriteCloser
	stdout *bufio.Reader
	stderr *bufio.Reader

	stderrMu   sync.Mutex
	stderrTail []string

	waitErr error
	done    chan struct{}
}

// Spawn starts the host and returns a Transport ready for Send/ReadLine. With debug, the
// host's stderr is forwarded to this process's stderr as well as kept for HostExit.
func Spawn(path string, debug bool, args ...string) (*Transport, error) {
	cmd := exec.Command(path, args...)
	stdinPipe, err := cmd.StdinPipe()
	if err != nil {
		return nil, fmt.Errorf("host %s: could not create stdin: %w", path, err)
	}
	stdoutPipe, err := cmd.StdoutPipe()
	if err != nil {
		return nil, fmt.Errorf("host %s: could not create stdout: %w", path, err)
	}
	stderrPipe, err := cmd.StderrPipe()
	if err != nil {
		return nil, fmt.Errorf("host %s: could not create stderr: %w", path, err)
	}
	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("host not found at %s: %w", path, err)
	}

	t := &Transport{
		proc:   cmd.Process,
		stdin:  stdinPipe,
		stdout: bufio.NewReader(stdoutPipe),
		stderr: bufio.NewReader(stderrPipe),
		done:   make(chan struct{}),
	}
	go t.drainStderr(debug)
	go t.wait()
	return t, nil
}

func (t *Transport) drainStderr(debug bool) {
	scanner := bufio.NewScanner(t.stderr)
	for scanner.Scan() {
		line := scanner.Text()
		if debug {
			fmt.Fprintln(os.Stderr, line)
		}
		t.stderrMu.Lock()
		t.stderrTail = append(t.stderrTail, line)
		if len(t.stderrTail) > stderrTailLines {
			t.stderrTail = t.stderrTail[1:]
		}
		t.stderrMu.Unlock()
	}
}

func (t *Transport) wait() {
	defer close(t.done)
	t.state, t.waitErr = t.proc.Wait()
}

// Send writes a client message as one JSON line ending in \n.
func (t *Transport) Send(message ClientMessage) error {
	line, err := json.Marshal(message)
	if err != nil {
		return err
	}
	_, err = t.stdin.Write(append(line, '\n'))
	return err
}

// SendRaw writes an already serialized line (useful to replay transcripts byte for byte).
func (t *Transport) SendRaw(line string) error {
	_, err := t.stdin.Write(append([]byte(line), '\n'))
	return err
}

// ReadLine waits for the host's next output line.
func (t *Transport) ReadLine() (string, error) {
	line, err := t.stdout.ReadString('\n')
	if err != nil {
		return "", err
	}
	return strings.TrimRight(line, "\r\n"), nil
}

// ReadLineTimeout waits at most d for the host's next output line.
func (t *Transport) ReadLineTimeout(d time.Duration) (string, error) {
	type res struct {
		line string
		err  error
	}
	ch := make(chan res, 1)
	go func() {
		line, err := t.stdout.ReadString('\n')
		ch <- res{strings.TrimRight(line, "\r\n"), err}
	}()
	select {
	case r := <-ch:
		return r.line, r.err
	case <-time.After(d):
		return "", fmt.Errorf("timed out waiting for host output")
	}
}

// Close closes stdin; the host exits on EOF.
func (t *Transport) Close() error {
	return t.stdin.Close()
}

// Wait waits for the host to exit and returns its HostExit.
func (t *Transport) Wait() HostExit {
	<-t.done
	t.stderrMu.Lock()
	tail := append([]string{}, t.stderrTail...)
	t.stderrMu.Unlock()

	exit := HostExit{StderrTail: tail}
	if t.state != nil && t.state.Exited() {
		code := t.state.ExitCode()
		exit.Code = &code
	}
	if t.waitErr != nil {
		exit.SpawnError = t.waitErr
	}
	return exit
}

// Host → client messages (key `type`), parsed into a map to compare with the
// transcripts and to dispatch in App.
func parseHostMessage(line string) (map[string]any, error) {
	var msg map[string]any
	if err := json.Unmarshal([]byte(line), &msg); err != nil {
		return nil, err
	}
	return msg, nil
}

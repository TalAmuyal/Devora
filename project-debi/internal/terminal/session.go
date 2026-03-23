package terminal

import (
	"devora/internal/process"
	"fmt"
	"os"
	"time"
)

type Session struct {
	ID       string
	Name     string
	RootPath string
}

type CommandRunner interface {
	GetOutput(command []string) (string, error)
	GetShellOutput(command string) (string, error)
}

type EnvGetter func(key string) string

type Backend interface {
	ListSessions() ([]Session, error)
	Attach(sessionID string) error
	CreateAndAttach(sessionName, workingDirectory, app string) error
}

type productionCommandRunner struct{}

func (p *productionCommandRunner) GetOutput(command []string) (string, error) {
	return process.GetOutput(command)
}

func (p *productionCommandRunner) GetShellOutput(command string) (string, error) {
	return process.GetShellOutput(command)
}

func NewBackend() Backend {
	runner := &productionCommandRunner{}
	return &KittyBackend{Runner: runner, GetEnv: os.Getenv}
}

func GetSessionByWorkingDirectory(backend Backend, workingDirectory string) (*Session, error) {
	sessions, err := backend.ListSessions()
	if err != nil {
		return nil, err
	}
	for _, s := range sessions {
		if s.RootPath == workingDirectory {
			return &s, nil
		}
	}
	return nil, nil
}

func Attach(backend Backend, workingDirectory string) (*Session, error) {
	session, err := GetSessionByWorkingDirectory(backend, workingDirectory)
	if err != nil {
		return nil, err
	}
	if session == nil {
		return nil, nil
	}
	err = backend.Attach(session.ID)
	if err != nil {
		return nil, err
	}
	return session, nil
}

func CreateAndAttach(backend Backend, sessionName, workingDirectory, app string, timeoutSeconds int) (*Session, error) {
	session, err := Attach(backend, workingDirectory)
	if err != nil {
		return nil, err
	}
	if session != nil {
		return session, nil
	}

	err = backend.CreateAndAttach(sessionName, workingDirectory, app)
	if err != nil {
		return nil, err
	}

	deadline := time.Now().Add(time.Duration(timeoutSeconds) * time.Second)
	for time.Now().Before(deadline) {
		session, err = GetSessionByWorkingDirectory(backend, workingDirectory)
		if err != nil {
			return nil, err
		}
		if session != nil {
			return session, nil
		}
		time.Sleep(100 * time.Millisecond)
	}

	return nil, fmt.Errorf("failed to create session: timed out after %d seconds", timeoutSeconds)
}

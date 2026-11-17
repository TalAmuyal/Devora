#!/usr/bin/env bash
# Runs a command holding the host-wide UI-test lock, so that runs which launch an app or drive the real pointer and keyboard
# (XCUITests, acceptance and e2e runs) queue instead of breaking each other when several agents share one Mac.
# Usage: with-ui-lock.sh <command> [args...]
# A run that finds the lock held says once what holds it, then waits; it never fails for it.
# The lock is a kernel flock (macOS has no flock command, so perl takes it), held by this wrapper's process while it runs
# the command as its child: whatever ends the wrapper, kill -9 included, releases it at once. Its descriptor is close-on-exec,
# so neither the command nor an app it leaves behind can keep it. Within a held run, nested calls (a stress task running a
# test task) pass straight through: DEVORA_UI_LOCK_HELD names the holder's PID, which must be alive and recorded in the lock.
# The lock file is /tmp/devora-ui-tests.lock (DEVORA_UI_LOCK replaces it), reachable by every agent of the same macOS user.
set -euo pipefail
if [ $# -eq 0 ]; then
	echo "usage: with-ui-lock.sh <command> [args...]" >&2
	exit 2
fi
DEVORA_UI_LOCK_WORKTREE=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
export DEVORA_UI_LOCK_WORKTREE
exec perl -e '
	use strict;
	use warnings;
	use Fcntl qw(:flock O_RDWR O_CREAT);
	use POSIX qw(strftime);
	my $path = $ENV{DEVORA_UI_LOCK} // "/tmp/devora-ui-tests.lock";
	my @command = @ARGV;

	# The child runs the command; this process keeps the lock until it ends. Ctrl-C reaches the child through the
	# terminal already, so only signals sent to this process alone are passed on.
	sub run_command {
		my $child = fork() // die "with-ui-lock: fork: $!\n";
		if ($child == 0) {
			exec { $command[0] } @command or die "with-ui-lock: $command[0]: $!\n";
		}
		$SIG{INT} = "IGNORE";
		$SIG{$_} = do { my $signal = $_; sub { kill $signal, $child } } for qw(TERM HUP);
		waitpid($child, 0);
		my $status = $?;
		return $status & 127 ? 128 + ($status & 127) : $status >> 8;
	}

	sub holder_pid {
		open(my $file, "<", $path) or return 0;
		my $first = <$file> // "";
		return $first =~ /^pid (\d+)$/ ? $1 : 0;
	}

	my $outer = $ENV{DEVORA_UI_LOCK_HELD} // "";
	if ($outer =~ /^\d+$/ && $outer > 0 && kill(0, $outer) && holder_pid() == $outer) {
		exit run_command();
	}

	sysopen(my $lock, $path, O_RDWR | O_CREAT, 0644) or die "with-ui-lock: $path: $!\n";
	unless (flock($lock, LOCK_EX | LOCK_NB)) {
		seek($lock, 0, 0);
		my $holder = do { local $/; <$lock> } // "";
		$holder =~ s/^pid \d+\n//;
		$holder =~ s/^/  /mg;
		print STDERR "with-ui-lock: waiting for the UI-test lock ($path), held by:\n$holder";
		flock($lock, LOCK_EX) or die "with-ui-lock: $path: $!\n";
		print STDERR "with-ui-lock: got the UI-test lock\n";
	}
	truncate($lock, 0);
	seek($lock, 0, 0);
	syswrite($lock, sprintf("pid %d\nworktree %s\ncommand %s\nsince %s\n",
		$$, $ENV{DEVORA_UI_LOCK_WORKTREE}, join(" ", @command), strftime("%Y-%m-%d %H:%M:%S", localtime)));
	$ENV{DEVORA_UI_LOCK_HELD} = $$;
	exit run_command();
' -- "$@"

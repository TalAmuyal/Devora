/// Folds a turn's start or end (`ChatUpdate.turn`) into which turn a chat shows running, by the core's rule
/// (`devora-state`'s `ChatUpdate::Turn`): a start of the agent's own turn doesn't replace one shown (a prompt sent just
/// before it began joined it), and an end ends only a turn of its own kind, so a late end of the agent's turn never ends
/// the user's.
func foldTurn(running: inout Bool, agentTurn: inout Bool, started: Bool, byAgent: Bool) {
	if started && byAgent {
		if !running { (running, agentTurn) = (true, true) }
	} else if started {
		(running, agentTurn) = (true, false)
	} else if agentTurn == byAgent {
		(running, agentTurn) = (false, false)
	}
}

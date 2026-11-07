import CoreBindings

extension Store {
	public func agentSettings(profilePath: String?) async -> AgentSettings? {
		guard let core else { return nil }
		return try? await call("get_agent_settings") { try core.getAgentSettings(profilePath: profilePath) }
	}

	public func setAgentSetting(profilePath: String?, key: String, value: String?) async -> Bool {
		guard let core else { return false }
		return (try? await call("set_agent_setting") {
			try core.setAgentSetting(profilePath: profilePath, key: key, value: value)
		}) != nil
	}

	public func setAgentInput(profilePath: String?, previousKey: String?, key: String, value: String?) async -> Bool {
		guard let core else { return false }
		return (try? await call("set_agent_input") {
			try core.setAgentInput(profilePath: profilePath, previousKey: previousKey, key: key, value: value)
		}) != nil
	}
}

We are now working the hooks mechanism.
We start in planning mode.
Please make a simple plan and show it to me.
The plan should describe the rest of the test.
The submission of the plan should invoke the plan-exit hook ("PermissionRequest" with "ExitPlanMode").
Once I approve the plan, execute it.

The plan should include the following:
1. Ask me a question to check the `AskUserQuestion` hook.
2. Make a minimal foo-bar.md file to check the `Write` hook.
3. Edit the new foo-bar.md file.
4. Run an `echo` command using the Bash tool.

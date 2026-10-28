-- Loaded by the host before the user's config (`--cmd luafile …`). Exposes `DevoraHost` for the host's RPC calls.
local M = {}

-- vim-tmux-navigator style: move between Neovim windows, and tell the host when there is no window that way.
local function navigate(dir)
	local before = vim.api.nvim_get_current_win()
	vim.cmd("wincmd " .. dir)
	if vim.api.nvim_get_current_win() == before then
		vim.rpcnotify(0, "devora_edge", dir)
	end
end

local function map_navigation()
	for _, dir in ipairs({ "h", "j", "k", "l" }) do
		-- Not insert mode: there <C-h> is backspace and <C-j> a newline.
		vim.keymap.set({ "n", "x" }, "<C-" .. dir .. ">", function() navigate(dir) end, { desc = "Devora: pane " .. dir })
		vim.keymap.set("n", "<C-w>" .. dir, function() navigate(dir) end, { desc = "Devora: pane " .. dir })
		vim.keymap.set("n", "<C-w><C-" .. dir .. ">", function() navigate(dir) end, { desc = "Devora: pane " .. dir })
	end
end

-- Loaded with --cmd, before the user's config: Neovim serves RPC requests while that config is still running, so
-- the host must not touch the editor until VimEnter. Mapping there also lets Devora's navigation win over the user's.
vim.api.nvim_create_autocmd("VimEnter", {
	once = true,
	callback = function()
		map_navigation()
		vim.rpcnotify(0, "devora_ready")
	end,
})

-- The prompt buffer: never a swap file (a kill -9 would leave an ATTENTION prompt behind).
function M.setup_draft_buffer()
	vim.bo.swapfile = false
	vim.bo.bufhidden = "hide"
	vim.bo.filetype = "markdown"
end

-- Replaces the buffer with a saved draft without recording the replacement as an undoable change, then loads the
-- undo history that was saved for exactly this text.
function M.restore(lines, row, col, undofile)
	local old = vim.o.undolevels
	vim.o.undolevels = -1
	vim.api.nvim_buf_set_lines(0, 0, -1, false, lines)
	vim.o.undolevels = old
	vim.bo.modified = false
	if vim.fn.filereadable(undofile) == 1 then
		vim.cmd("silent! rundo " .. vim.fn.fnameescape(undofile))
	end
	pcall(vim.api.nvim_win_set_cursor, 0, { row, col })
end

function M.save_undo(undofile)
	vim.cmd("silent! wundo! " .. vim.fn.fnameescape(undofile))
end

DevoraHost = M

local M = {}

local util = require("whence.util")
local NS = vim.api.nvim_create_namespace("whence")

local BUF_OPTIONS = {
  buftype = "nofile",
  bufhidden = "hide",
  swapfile = false,
  filetype = "whence",
  shiftwidth = 2,
}

local WIN_OPTIONS = {
  foldmethod = "indent",
  foldlevel = 99,
  number = false,
  relativenumber = false,
  wrap = false,
}

local state = { buf = nil, index = {}, ctx = {} }

local function present(value)
  if value == nil or value == vim.NIL then
    return nil
  end
  return value
end

function M.render(tree, root)
  local lines, index = {}, {}

  local function walk(node, depth)
    local pad = string.rep("  ", depth)
    local stop = present(node.stop)
    local via = present(node.via)
    local viaText = via and ("  ← " .. via) or ""
    local stopText = ""
    if stop then
      local detail = present(stop.detail)
      stopText = "  [" .. stop.reason .. (detail and (": " .. detail) or "") .. "]"
    end
    local marker = stop and "■ " or "● "
    local loc = node.loc

    lines[#lines + 1] = pad
      .. marker
      .. node.label
      .. viaText
      .. "  "
      .. util.rel(loc.file, root)
      .. ":"
      .. (loc.line + 1)
      .. ":"
      .. (loc.col + 1)
      .. stopText
    index[#lines] = node

    for _, child in ipairs(node.children or {}) do
      walk(child, depth + 1)
    end

    local truncated = present(node.truncated) or 0
    if truncated > 0 then
      lines[#lines + 1] = string.rep("  ", depth + 1) .. "… " .. truncated .. " more"
    end
  end

  if tree and tree.root then
    walk(tree.root, 0)
  end
  return lines, index
end

local function define_highlights()
  vim.api.nvim_set_hl(0, "WhenceStop", { link = "DiagnosticWarn", default = true })
  vim.api.nvim_set_hl(0, "WhenceTrunc", { link = "Comment", default = true })
  vim.api.nvim_set_hl(0, "WhenceLoc", { link = "Directory", default = true })
end

local function ensure_buf()
  if state.buf and vim.api.nvim_buf_is_valid(state.buf) then
    return state.buf
  end

  local buf = vim.api.nvim_create_buf(false, true)
  pcall(vim.api.nvim_buf_set_name, buf, "whence://provenance")
  for name, value in pairs(BUF_OPTIONS) do
    vim.bo[buf][name] = value
  end

  local opts = { buffer = buf, nowait = true, silent = true }
  vim.keymap.set("n", "<CR>", M.jump_current, vim.tbl_extend("force", opts, { desc = "whence: jump" }))
  vim.keymap.set("n", "p", M.preview_current, vim.tbl_extend("force", opts, { desc = "whence: preview" }))
  vim.keymap.set("n", "R", M.rerun_current, vim.tbl_extend("force", opts, { desc = "whence: re-run" }))
  vim.keymap.set("n", "q", M.close, vim.tbl_extend("force", opts, { desc = "whence: close" }))

  state.buf = buf
  return buf
end

local function panel_win(buf)
  for _, win in ipairs(vim.api.nvim_list_wins()) do
    if vim.api.nvim_win_get_buf(win) == buf then
      return win
    end
  end
  return nil
end

local function ensure_win(buf, width)
  local win = panel_win(buf)
  if win then
    return win
  end
  vim.cmd("botright vsplit")
  win = vim.api.nvim_get_current_win()
  vim.api.nvim_win_set_buf(win, buf)
  vim.api.nvim_win_set_width(win, width or 60)
  for name, value in pairs(WIN_OPTIONS) do
    vim.wo[win][name] = value
  end
  return win
end

function M.show(tree, ctx)
  ctx = ctx or {}
  define_highlights()

  local lines, index = M.render(tree, ctx.root)
  local buf = ensure_buf()
  state.index[buf] = index
  state.ctx[buf] = ctx

  vim.bo[buf].modifiable = true
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].modifiable = false

  vim.api.nvim_buf_clear_namespace(buf, NS, 0, -1)
  for i, node in pairs(index) do
    local snippet = present(node.snippet)
    if snippet and snippet ~= "" then
      vim.api.nvim_buf_set_extmark(buf, NS, i - 1, 0, {
        virt_text = { { snippet, "Comment" } },
        virt_text_pos = "eol",
      })
    end
  end

  local win = ensure_win(buf, ctx.width)
  vim.api.nvim_set_current_win(win)
  if #lines > 0 then
    vim.api.nvim_win_set_cursor(win, { 1, 0 })
  end

  M.last = { tree = tree, ctx = ctx }
end

local function current_node()
  local buf = vim.api.nvim_get_current_buf()
  local index = state.index[buf]
  if not index then
    return nil, buf
  end
  return index[vim.api.nvim_win_get_cursor(0)[1]], buf
end

local function usable(win, buf)
  return win
    and win ~= 0
    and vim.api.nvim_win_is_valid(win)
    and vim.api.nvim_win_get_buf(win) ~= buf
end

local function target_win(buf)
  local ctx = state.ctx[buf] or {}
  if usable(ctx.source_win, buf) then
    return ctx.source_win
  end
  local prev = vim.fn.win_getid(vim.fn.winnr("#"))
  if usable(prev, buf) then
    return prev
  end
  vim.cmd("aboveleft vsplit")
  return vim.api.nvim_get_current_win()
end

local function goto_node(node, buf, centre, keep_focus)
  local panel = vim.api.nvim_get_current_win()
  local file = node.loc.file
  if not vim.startswith(file, "/") then
    file = vim.fs.joinpath((state.ctx[buf] or {}).root or vim.fn.getcwd(), file)
  end

  vim.api.nvim_set_current_win(target_win(buf))
  if vim.api.nvim_buf_get_name(0) ~= file then
    vim.cmd.edit(vim.fn.fnameescape(file))
  end

  local line = math.min(node.loc.line + 1, vim.api.nvim_buf_line_count(0))
  local text = vim.api.nvim_buf_get_lines(0, line - 1, line, false)[1] or ""
  vim.api.nvim_win_set_cursor(0, { line, util.byte_col(text, node.loc.col) })

  if centre then
    vim.cmd("normal! zz")
  end
  if keep_focus and vim.api.nvim_win_is_valid(panel) then
    vim.api.nvim_set_current_win(panel)
  end
end

function M.jump_current()
  local node, buf = current_node()
  if node then
    goto_node(node, buf, false, false)
  end
end

function M.preview_current()
  local node, buf = current_node()
  if node then
    goto_node(node, buf, true, true)
  end
end

function M.rerun_current()
  local node = current_node()
  if node then
    require("whence").trace_at(node.loc.file, node.loc.line, node.loc.col)
  end
end

function M.close()
  local win = panel_win(state.buf)
  if win and #vim.api.nvim_list_wins() > 1 then
    vim.api.nvim_win_close(win, true)
  end
end

return M

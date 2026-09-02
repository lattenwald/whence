local M = {}

local engine = require("whence.engine")
local util = require("whence.util")

local ROOT_MARKERS = { "rebar.config", "Cargo.toml", "go.mod", ".git" }

local config = { bin = nil, limits = {}, panel = { width = 60 } }
local clients = {}

local function notify(msg, level)
  vim.notify("whence: " .. msg, level or vim.log.levels.ERROR)
end

local function find_bin()
  local candidates = {
    config.bin or "",
    vim.g.whence_bin or "",
    vim.fn.exepath("whence"),
    vim.fn.stdpath("data") .. "/whence/bin/whence",
  }
  for _, c in ipairs(candidates) do
    if c ~= "" and vim.fn.executable(c) == 1 then
      return c
    end
  end
  return nil
end

local function root_of(file)
  if config.root then
    return config.root
  end
  return vim.fs.root(file or 0, ROOT_MARKERS) or vim.fn.getcwd()
end

local function client_for(root)
  if clients[root] and not clients[root].is_closing() then
    return clients[root]
  end
  clients[root] = nil

  local bin = find_bin()
  if not bin then
    notify("engine binary not found; run :WhenceInstall or set vim.g.whence_bin")
    return nil
  end

  local client, err = engine.start({
    cmd = { bin, "serve" },
    root = root,
    on_exit = function()
      clients[root] = nil
    end,
  })
  if not client then
    notify(err)
    return nil
  end
  clients[root] = client
  return client
end

function M.setup(opts)
  config = vim.tbl_deep_extend("force", config, opts or {})
  -- Routed through host.handle so the recorder, which wraps it, sees replayed answers too.
  if config._replay then
    require("whence.host").handle = require("whence.host_replay").handle(config._replay)
  end
end

function M.root(file)
  return root_of(file)
end

function M.trace_at(file, line, col, on_done)
  on_done = on_done or function() end
  local root = root_of(file)
  local client = client_for(root)
  if not client then
    on_done("engine unavailable")
    return
  end
  local source_win = vim.api.nvim_get_current_win()
  local host = require("whence.host")
  host.reset()
  engine.trace(client, { file = file, line = line, col = col, limits = config.limits }, function(err, tree)
    host.reset()
    if err then
      notify(err.message or vim.inspect(err))
      on_done(err)
      return
    end
    -- Before the render, so a panel error cannot strand a recorder that wraps the host.
    on_done(nil)
    require("whence.panel").show(tree, {
      source_win = source_win,
      root = root,
      limits = config.limits,
      width = (config.panel or {}).width,
    })
  end)
end

function M.trace(on_done)
  local target = util.cursor_target()
  if not target then
    notify("buffer has no file")
    if on_done then
      on_done("buffer has no file")
    end
    return
  end
  M.trace_at(target.file, target.line, target.col, on_done)
end

function M.stop()
  for root, client in pairs(clients) do
    engine.stop(client)
    clients[root] = nil
  end
end

vim.api.nvim_create_autocmd("VimLeavePre", {
  group = vim.api.nvim_create_augroup("whence", { clear = true }),
  callback = function()
    for _, client in pairs(clients) do
      client.terminate()
    end
    clients = {}
  end,
})

return M

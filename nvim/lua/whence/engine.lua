local M = {}

-- Neovim asserts server_request error codes are in vim.lsp.protocol.ErrorCodes,
-- which excludes the engine's own -32000 (E_HOST); only the message reaches it.
local INTERNAL_ERROR = -32603
local E_HOST = -32000

local function host_dispatcher(handler)
  return function(method, params)
    local ok, result, err = pcall(handler, method, params)
    if not ok then
      return nil, { code = INTERNAL_ERROR, message = tostring(result) }
    end
    if result == nil and err == nil then
      return nil, { code = INTERNAL_ERROR, message = "no answer for " .. method }
    end
    return result, err
  end
end

-- vim.lsp.rpc drops pending callbacks when the process dies, so they are failed here.
local function fail_pending(state, code)
  local pending = state.pending
  state.pending = {}
  if not next(pending) and code == 0 then
    return
  end
  vim.schedule(function()
    vim.notify("whence: engine exited " .. tostring(code), vim.log.levels.ERROR)
    for _, cb in pairs(pending) do
      cb({ code = E_HOST, message = "engine exited" }, nil)
    end
  end)
end

function M.start(opts)
  -- Resolved per request so a recorder installed after the client started still sees them.
  local handler = opts.handle or function(method, params)
    return require("whence.host").handle(method, params)
  end
  local state = { pending = {} }
  local ok, rpc = pcall(vim.lsp.rpc.start, opts.cmd, {
    server_request = host_dispatcher(handler),
    notification = function() end,
    on_error = function(code, err)
      vim.notify("whence: rpc error " .. tostring(code) .. " " .. vim.inspect(err), vim.log.levels.ERROR)
    end,
    on_exit = function(code)
      fail_pending(state, code)
      if opts.on_exit then
        opts.on_exit(code)
      end
    end,
  }, { cwd = opts.root })
  if not ok or not rpc then
    return nil, "failed to start " .. table.concat(opts.cmd, " ") .. (ok and "" or ": " .. tostring(rpc))
  end
  local client = {
    request = rpc.request,
    notify = rpc.notify,
    is_closing = rpc.is_closing,
    terminate = rpc.terminate,
    _state = state,
  }

  local done, ierr, info = false, nil, nil
  client.request("initialize", { root = opts.root, capabilities = { documentHighlight = true } }, function(e, r)
    ierr, info, done = e, r, true
  end)
  if not vim.wait(5000, function()
    return done
  end) then
    client.terminate()
    return nil, "engine did not answer initialize"
  end
  if ierr then
    client.terminate()
    return nil, vim.inspect(ierr)
  end
  return client, nil, info
end

function M.trace(client, params, cb)
  local state = client._state
  local id, fired = nil, false
  local function done(err, result)
    if fired then
      return
    end
    fired = true
    if id then
      state.pending[id] = nil
    end
    cb(err, result)
  end
  local sent, request_id = client.request("whence/trace", params, done)
  if not sent then
    done({ code = E_HOST, message = "engine exited" }, nil)
    return
  end
  id = request_id
  if not fired then
    state.pending[id] = done
  end
end

-- The engine answers shutdown "busy" while a trace runs; terminate() closes
-- stdin, and the EOF ends its loop even mid-trace.
function M.stop(client)
  if not client or client.is_closing() then
    return
  end
  local acked = false
  client.request("shutdown", vim.empty_dict(), function()
    acked = true
    client.terminate()
  end)
  vim.wait(2000, function()
    return acked
  end)
  if not acked then
    client.terminate()
  end
end

return M

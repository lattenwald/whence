local M = {}

-- Neovim asserts server_request error codes are in vim.lsp.protocol.ErrorCodes,
-- which excludes the engine's own -32000 (E_HOST); only the message reaches it.
local INTERNAL_ERROR = -32603

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

function M.start(opts)
  local handler = opts.handle or require("whence.host").handle
  local ok, client = pcall(vim.lsp.rpc.start, opts.cmd, {
    server_request = host_dispatcher(handler),
    notification = function() end,
    on_error = function(code, err)
      vim.notify("whence: rpc error " .. tostring(code) .. " " .. vim.inspect(err), vim.log.levels.ERROR)
    end,
    on_exit = function(code)
      if opts.on_exit then
        opts.on_exit(code)
      end
    end,
  }, { cwd = opts.root })
  if not ok or not client then
    return nil, "failed to start " .. table.concat(opts.cmd, " ") .. (ok and "" or ": " .. tostring(client))
  end

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
  client.request("whence/trace", params, function(err, result)
    cb(err, result)
  end)
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

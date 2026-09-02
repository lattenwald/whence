local M = {}

local METHOD_NOT_FOUND = -32601
local INTERNAL_ERROR = -32603

local function absolutise(dir, locations)
  for _, loc in ipairs(locations) do
    if not vim.startswith(loc.file, "/") then
      loc.file = dir .. "/" .. loc.file
    end
  end
  return locations
end

local function load(dir)
  local recorded = vim.json.decode(table.concat(vim.fn.readfile(dir .. "/host.json"), "\n"))
  for _, section in ipairs({ "definition", "references" }) do
    for _, locations in pairs(recorded[section] or {}) do
      absolutise(dir, locations)
    end
  end
  return recorded
end

local function key(dir, params)
  local rel = params.file
  if vim.startswith(rel, dir .. "/") then
    rel = rel:sub(#dir + 2)
  end
  return ("%s:%d:%d"):format(rel, params.line, params.col)
end

-- Key scheme mirrors engine/src/host_replay.rs; fixtures are shared with it.
function M.handle(dir)
  local recorded = load(dir)
  return function(method, params)
    if method == "host/text" then
      return { text = table.concat(vim.fn.readfile(params.file), "\n") .. "\n" }
    end
    local section = method:match("^host/(.+)$")
    if not section or not recorded[section] then
      return nil, { code = METHOD_NOT_FOUND, message = "unknown method " .. tostring(method) }
    end
    local k = key(dir, params)
    if method == "host/references" then
      k = k .. (params.includeDeclaration and "|decl" or "|nodecl")
    end
    local answer = recorded[section][k]
    if not answer then
      return nil, { code = INTERNAL_ERROR, message = "unrecorded" }
    end
    return answer
  end
end

return M

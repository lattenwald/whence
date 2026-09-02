local M = {}

local host = require("whence.host")
local util = require("whence.util")

local SECTION = {
  ["host/definition"] = "definition",
  ["host/references"] = "references",
  ["host/documentHighlight"] = "documentHighlight",
}

local active = nil

local function relativised(rec, locations)
  local out = vim.deepcopy(locations)
  for _, loc in ipairs(out) do
    loc.file = util.rel(loc.file, rec.root)
  end
  return out
end

local function copy_source(rec, file, text)
  local rel = util.rel(file, rec.root)
  if rel == file then
    return
  end
  local path = rec.dir .. "/" .. rel
  vim.fn.mkdir(vim.fs.dirname(path), "p")
  -- "b" joins the lines with \n and adds no trailing newline, so this round-trips exactly.
  vim.fn.writefile(vim.split(text, "\n"), path, "b")
end

local function capture(rec, method, params, result)
  if method == "host/text" then
    copy_source(rec, params.file, result.text)
    return
  end
  local section = SECTION[method]
  if not section then
    return
  end
  local k = util.fixture_key(rec.root, method, params)
  -- Fallback only: the engine's first positional request is at the identifier it resolved.
  rec.target = rec.target or { file = util.rel(params.file, rec.root), line = params.line, col = params.col }

  local answer = section == "documentHighlight" and vim.deepcopy(result) or relativised(rec, result)
  local first = rec.recorded[section][k]
  if first ~= nil then
    if not vim.deep_equal(first, answer) and not rec.conflicts[k] then
      rec.conflicts[k] = true
      rec.conflicting[#rec.conflicting + 1] = method .. " " .. k
    end
    return
  end
  rec.recorded[section][k] = answer
end

function M.begin(dir, root, target)
  if active then
    error("whence: a recording into " .. active.dir .. " is already active")
  end
  if vim.fn.isdirectory(dir) == 1 and #vim.fn.readdir(dir) > 0 then
    error("whence: " .. dir .. " is not empty; record into a fresh directory")
  end
  vim.fn.mkdir(dir, "p")
  local rec = {
    dir = dir,
    root = root,
    recorded = { definition = {}, references = {}, documentHighlight = {} },
    conflicts = {},
    conflicting = {},
    target = target and { file = util.rel(target.file, root), line = target.line, col = target.col } or nil,
    orig = host.handle,
  }
  host.handle = function(method, params)
    local result, err = rec.orig(method, params)
    if not err and result ~= nil then
      capture(rec, method, params, result)
    end
    return result, err
  end
  active = rec
end

function M.finish()
  local rec = active
  if not rec then
    return nil
  end
  active = nil
  host.handle = rec.orig

  local sections = {}
  for name, entries in pairs(rec.recorded) do
    sections[name] = next(entries) and entries or vim.empty_dict()
  end
  vim.fn.writefile({ vim.json.encode(sections) }, rec.dir .. "/host.json")

  local meta = {
    root = rec.root,
    file = rec.target and rec.target.file or vim.NIL,
    line = rec.target and rec.target.line or vim.NIL,
    col = rec.target and rec.target.col or vim.NIL,
    engine_version = require("whence.version"),
    recorded_at = os.date("!%Y-%m-%dT%H:%M:%SZ"),
    conflicts = rec.conflicting,
  }
  vim.fn.writefile({ vim.json.encode(meta) }, rec.dir .. "/whence-record.json")

  if #rec.conflicting > 0 then
    vim.notify(
      "whence: the host answered these differently on a repeat; the fixture keeps the first answer and "
        .. "does not reproduce the session: "
        .. table.concat(rec.conflicting, ", "),
      vim.log.levels.WARN
    )
  end

  if not rec.target then
    return nil
  end
  return ("whence replay %s %s:%d:%d"):format(rec.dir, rec.target.file, rec.target.line + 1, rec.target.col + 1)
end

function M.run(dir, root)
  M.begin(dir, root, util.cursor_target())
  local finished = false
  local function done()
    if finished then
      return
    end
    finished = true
    local hint = M.finish()
    vim.notify("whence: " .. (hint or ("recorded nothing into " .. dir)))
  end
  local ok, err = pcall(require("whence").trace, done)
  if not ok then
    done()
    error(err)
  end
end

return M

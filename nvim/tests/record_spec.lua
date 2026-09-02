local fx = vim.fn.getcwd() .. "/engine/tests/fixtures/erlang/local_chain"
local pc = vim.fn.getcwd() .. "/engine/tests/fixtures/erlang/param_callers"

local function read_json(path)
  return vim.json.decode(table.concat(vim.fn.readfile(path), "\n"))
end

describe("record", function()
  it("writes a replayable fixture", function()
    local out = vim.fn.tempname()
    vim.fn.mkdir(out, "p")

    local whence = require("whence")
    whence.setup({ _replay = fx, root = fx })
    local record = require("whence.record")
    record.begin(out, fx)

    local panel = require("whence.panel")
    panel.last = nil
    whence.trace_at(fx .. "/a.erl", 6, 4)
    vim.wait(10000, function()
      return panel.last ~= nil
    end)
    record.finish()
    whence.stop()

    assert.is_not_nil(panel.last)
    local host = read_json(out .. "/host.json")
    assert.is_truthy(host.definition["a.erl:6:4"])
    assert.equals("a.erl", host.definition["a.erl:6:4"][1].file)
    assert.same({}, host.references["a.erl:3:0|nodecl"])
    assert.equals(1, vim.fn.filereadable(out .. "/a.erl"))

    local meta = read_json(out .. "/whence-record.json")
    assert.equals(fx, meta.root)
    assert.equals("a.erl", meta.file)
    assert.equals(6, meta.line)
    assert.equals(4, meta.col)
    assert.same({}, meta.conflicts)

    local res = vim.system({ vim.g.whence_bin, "replay", out, "a.erl:7:5" }):wait()
    assert.equals(0, res.code)
    assert.is_truthy(res.stdout:find("^Z"))
  end)

  it("records the cursor position, not the identifier the engine resolved", function()
    local out = vim.fn.tempname()
    local whence = require("whence")
    whence.setup({ _replay = pc, root = pc })
    vim.cmd.edit(pc .. "/c.erl")
    -- one byte into `Val` of `b:g(Val)`, which the engine resolves to its start at col 8
    vim.api.nvim_win_set_cursor(0, { 7, 9 })

    local panel = require("whence.panel")
    panel.last = nil
    require("whence.record").run(out)
    vim.wait(10000, function()
      return panel.last ~= nil
    end)
    whence.stop()

    local meta = read_json(out .. "/whence-record.json")
    assert.equals("c.erl", meta.file)
    assert.equals(6, meta.line)
    assert.equals(9, meta.col)

    local host = read_json(out .. "/host.json")
    assert.is_truthy(host.definition["c.erl:6:8"])
    local os_erl = host.definition["c.erl:4:13"][1].file
    assert.is_falsy(vim.startswith(os_erl, pc))
    assert.equals(0, vim.fn.filereadable(out .. os_erl))

    local res = vim.system({ vim.g.whence_bin, "replay", out, "c.erl:7:10" }):wait()
    assert.equals(0, res.code)
    assert.is_truthy(res.stdout:find("^Val"))
  end)

  it("keeps the first of two differing answers and flags the fixture", function()
    local host = require("whence.host")
    local record = require("whence.record")
    local out = vim.fn.tempname()
    local orig = host.handle
    local n = 0
    host.handle = function()
      n = n + 1
      return { { file = "/root/x.erl", range = { start = { line = n, col = 0 }, ["end"] = { line = n, col = 1 } } } }
    end

    record.begin(out, "/root")
    host.handle("host/definition", { file = "/root/a.erl", line = 1, col = 2 })
    host.handle("host/definition", { file = "/root/a.erl", line = 1, col = 2 })
    record.finish()
    host.handle = orig

    assert.equals(1, read_json(out .. "/host.json").definition["a.erl:1:2"][1].range.start.line)
    assert.same({ "host/definition a.erl:1:2" }, read_json(out .. "/whence-record.json").conflicts)
  end)

  it("refuses a non-empty directory", function()
    local record = require("whence.record")
    local out = vim.fn.tempname()
    vim.fn.mkdir(out, "p")
    vim.fn.writefile({ "old" }, out .. "/host.json")
    local ok, err = pcall(record.begin, out, fx)
    assert.is_false(ok)
    assert.is_truthy(tostring(err):find("not empty"))
  end)

  it("refuses to start a second recording", function()
    local record = require("whence.record")
    local out = vim.fn.tempname()
    record.begin(out, fx)
    local ok, err = pcall(record.begin, vim.fn.tempname(), fx)
    record.finish()
    assert.is_false(ok)
    assert.is_truthy(tostring(err):find("already"))
  end)

  it("relativises only paths under the root", function()
    local record = require("whence.record")
    assert.equals("a.erl", require("whence.util").rel("/root/a.erl", "/root"))
    assert.equals("sub/a.erl", require("whence.util").rel("/root/sub/a.erl", "/root"))
    assert.equals("/other/a.erl", require("whence.util").rel("/other/a.erl", "/root"))
  end)
end)

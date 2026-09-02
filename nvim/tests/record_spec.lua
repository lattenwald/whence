local fx = vim.fn.getcwd() .. "/engine/tests/fixtures/erlang/local_chain"

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
    local host = vim.json.decode(table.concat(vim.fn.readfile(out .. "/host.json"), "\n"))
    assert.is_truthy(host.definition["a.erl:6:4"])
    assert.equals("a.erl", host.definition["a.erl:6:4"][1].file)
    assert.same({}, host.references["a.erl:3:0|nodecl"])
    assert.equals(1, vim.fn.filereadable(out .. "/a.erl"))

    local meta = vim.json.decode(table.concat(vim.fn.readfile(out .. "/whence-record.json"), "\n"))
    assert.equals(fx, meta.root)
    assert.equals("a.erl", meta.file)
    assert.equals(6, meta.line)
    assert.equals(4, meta.col)

    local res = vim.system({ vim.g.whence_bin, "replay", out, "a.erl:7:5" }):wait()
    assert.equals(0, res.code)
    assert.is_truthy(res.stdout:find("^Z"))
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
    assert.equals("a.erl", record._rel("/root/a.erl", "/root"))
    assert.equals("sub/a.erl", record._rel("/root/sub/a.erl", "/root"))
    assert.equals("/other/a.erl", record._rel("/other/a.erl", "/root"))
  end)
end)

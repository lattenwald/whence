local fx = vim.fn.getcwd() .. "/engine/tests/fixtures/erlang/local_chain"

describe("engine", function()
  it("initializes and traces through the replay server", function()
    local engine = require("whence.engine")
    local client = assert(engine.start({ cmd = { vim.g.whence_bin, "replay", "--serve", fx }, root = fx }))

    local done, tree, err = false, nil, nil
    engine.trace(client, { file = fx .. "/a.erl", line = 6, col = 4 }, function(e, t)
      err, tree, done = e, t, true
    end)
    vim.wait(5000, function()
      return done
    end)

    assert.is_nil(err)
    assert.equals("Z", tree.root.label)
    assert.equals("binding", tree.root.kind)
    assert.equals("Y", tree.root.children[1].label)
    engine.stop(client)
  end)

  it("fails a pending trace when the engine dies", function()
    local engine = require("whence.engine")
    local client = assert(engine.start({ cmd = { vim.g.whence_bin, "replay", "--serve", fx }, root = fx }))

    local err, done = nil, false
    engine.trace(client, { file = fx .. "/a.erl", line = 6, col = 4 }, function(e)
      err, done = e, true
    end)
    client.terminate()

    assert.is_true(vim.wait(2000, function()
      return done
    end))
    assert.equals("engine exited", err.message)
  end)

  it("reports a missing binary instead of throwing", function()
    local engine = require("whence.engine")
    local client, err = engine.start({ cmd = { "/nonexistent/whence" }, root = fx })
    assert.is_nil(client)
    assert.is_not_nil(err)
  end)
end)

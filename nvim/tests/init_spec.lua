local fx = vim.fn.getcwd() .. "/engine/tests/fixtures/erlang/local_chain"

describe("whence.setup", function()
  it("traces through a real `whence serve` with the replay host", function()
    local whence = require("whence")
    whence.setup({ _replay = fx, root = fx })

    local panel = require("whence.panel")
    panel.last = nil
    whence.trace_at(fx .. "/a.erl", 6, 4)
    vim.wait(10000, function()
      return panel.last ~= nil
    end)

    assert.is_not_nil(panel.last)
    assert.equals("Z", panel.last.tree.root.label)
    whence.stop()
  end)
end)

describe("whence.root", function()
  before_each(function()
    -- A fresh module: an earlier test pinned `root` through setup().
    package.loaded["whence"] = nil
  end)

  local function fake_server(dispatchers)
    local closing = false
    return {
      request = function(method, _, cb)
        if method == "initialize" then
          cb(nil, { capabilities = {} })
        elseif method == "shutdown" then
          cb(nil, nil)
        end
        return true, 1
      end,
      notify = function()
        return true
      end,
      is_closing = function()
        return closing
      end,
      terminate = function()
        closing = true
        dispatchers.on_exit(0, 0)
      end,
    }
  end

  it("is the attached language server's root", function()
    local root = vim.fn.tempname()
    vim.fn.mkdir(root .. "/deep", "p")
    local file = root .. "/deep/x.txt"
    vim.fn.writefile({ "x" }, file)
    local b = vim.fn.bufadd(file)
    vim.fn.bufload(b)
    local id = vim.lsp.start({ name = "fake", cmd = fake_server, root_dir = root }, { bufnr = b })
    assert.is_truthy(id)
    vim.wait(2000, function()
      return #vim.lsp.get_clients({ bufnr = b }) > 0
    end)
    assert.equals(root, require("whence").root(file))
    vim.lsp.stop_client(id)
  end)

  it("falls back to the git root, then the cwd", function()
    local root = vim.fn.tempname()
    vim.fn.mkdir(root .. "/.git/sub", "p")
    local file = root .. "/sub/y.txt"
    vim.fn.mkdir(root .. "/sub", "p")
    vim.fn.writefile({ "y" }, file)
    assert.equals(root, require("whence").root(file))

    local loose = vim.fn.tempname() .. ".txt"
    vim.fn.writefile({ "z" }, loose)
    assert.equals(vim.fn.getcwd(), require("whence").root(loose))
  end)
end)

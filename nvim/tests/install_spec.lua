describe("install", function()
  local install = require("whence.install")

  it("maps every platform to a target the release workflow builds, and nothing else", function()
    local released = {}
    for _, l in ipairs(vim.fn.readfile(vim.fn.getcwd() .. "/.github/workflows/release.yml")) do
      local t = l:match("^%s*%- target:%s*(%S+)")
      if t then
        released[t] = true
      end
    end
    local mapped = {}
    for _, os_name in ipairs({ "Linux", "OSX", "Windows" }) do
      for _, arch in ipairs({ "x64", "arm64" }) do
        local t = install.target(os_name, arch)
        if t then
          mapped[t] = true
        end
      end
    end
    assert.same(released, mapped)
  end)

  it("reports an unsupported platform instead of guessing", function()
    local target, err = install.target("Linux", "ppc64")
    assert.is_nil(target)
    assert.is_truthy(err:find("ppc64"))
  end)

  it("parses SHA256SUMS", function()
    local sums = install.parse_sums(table.concat({
      "aa11  whence-x86_64-unknown-linux-gnu.tar.gz",
      "bb22 *whence-x86_64-pc-windows-msvc.tar.gz",
      "",
      "# a comment",
    }, "\n"))
    assert.equals("aa11", sums["whence-x86_64-unknown-linux-gnu.tar.gz"])
    assert.equals("bb22", sums["whence-x86_64-pc-windows-msvc.tar.gz"])
    assert.is_nil(sums["# a comment"])
  end)

  it("hashes a binary exactly like sha256sum", function()
    assert.equals(1, vim.fn.executable("sha256sum"))
    local out = vim.system({ "sha256sum", vim.g.whence_bin }):wait().stdout
    assert.equals(out:match("^(%x+)"), install.sha256_file(vim.g.whence_bin))
  end)

  it("reports a missing file instead of throwing", function()
    local sha, err = install.sha256_file("/nonexistent/whence.tar.gz")
    assert.is_nil(sha)
    assert.is_truthy(err:find("/nonexistent/whence.tar.gz"))
  end)

  it("ships a version matching the engine crate", function()
    local crate = table.concat(vim.fn.readfile(vim.fn.getcwd() .. "/engine/Cargo.toml"), "\n")
    assert.equals(crate:match('\nversion%s*=%s*"([^"]+)"'), require("whence.version"))
  end)
end)

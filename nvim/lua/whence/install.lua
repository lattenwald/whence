local M = {}

local TARGETS = {
  Linux = { x64 = "x86_64-unknown-linux-gnu", arm64 = "aarch64-unknown-linux-gnu" },
  OSX = { x64 = "x86_64-apple-darwin", arm64 = "aarch64-apple-darwin" },
  Windows = { x64 = "x86_64-pc-windows-msvc" },
}

function M.target(os_name, arch)
  os_name = os_name or jit.os
  arch = arch or jit.arch
  local target = (TARGETS[os_name] or {})[arch]
  if not target then
    return nil, ("no published release for %s/%s"):format(os_name, arch)
  end
  return target
end

function M.urls(repo, version, target)
  local base = ("https://github.com/%s/releases/download/v%s"):format(repo, version)
  local archive = ("whence-%s.tar.gz"):format(target)
  return { archive = archive, tarball = base .. "/" .. archive, sums = base .. "/SHA256SUMS" }
end

function M.parse_sums(text)
  local out = {}
  for line in (text .. "\n"):gmatch("([^\n]*)\n") do
    local sha, name = line:match("^(%x+)%s+%*?(%S+)%s*$")
    if sha then
      out[name] = sha
    end
  end
  return out
end

local function run(cmd)
  local res = vim.system(cmd):wait()
  if res.code ~= 0 then
    return nil, table.concat(cmd, " ") .. ": " .. (res.stderr ~= "" and res.stderr or "exit " .. res.code)
  end
  return res.stdout
end

local function sha256(path)
  for _, cmd in ipairs({ { "sha256sum", path }, { "shasum", "-a", "256", path } }) do
    if vim.fn.executable(cmd[1]) == 1 then
      local out, err = run(cmd)
      return out and out:match("^(%x+)"), err
    end
  end
  return nil, "neither sha256sum nor shasum is on PATH"
end

local function download(url, path)
  local _, err = run({ "curl", "-fsSL", "-o", path, url })
  return err
end

function M.install(cb)
  cb = cb or function() end
  local target, err = M.target()
  if not target then
    return cb(err)
  end
  if vim.fn.executable("curl") == 0 or vim.fn.executable("tar") == 0 then
    return cb("curl and tar are required to install the engine")
  end

  local urls = M.urls(vim.g.whence_repo or "lattenwald/whence", require("whence.version"), target)
  local dir = vim.fn.stdpath("data") .. "/whence"
  local bin = dir .. "/bin"
  vim.fn.mkdir(bin, "p")

  local archive, sums = dir .. "/" .. urls.archive, dir .. "/SHA256SUMS"
  err = download(urls.tarball, archive) or download(urls.sums, sums)
  if err then
    return cb(err)
  end

  local want = M.parse_sums(table.concat(vim.fn.readfile(sums), "\n"))[urls.archive]
  if not want then
    return cb(urls.archive .. " is not listed in SHA256SUMS")
  end
  local got, sherr = sha256(archive)
  if not got then
    return cb(sherr)
  end
  if got:lower() ~= want:lower() then
    return cb("checksum mismatch for " .. urls.archive .. ": got " .. got .. ", expected " .. want)
  end

  local _, terr = run({ "tar", "-xzf", archive, "-C", bin })
  if terr then
    return cb(terr)
  end
  local exe = bin .. "/whence" .. (target:find("windows") and ".exe" or "")
  if vim.fn.filereadable(exe) == 0 then
    return cb(urls.archive .. " did not contain " .. vim.fs.basename(exe))
  end
  if not target:find("windows") then
    run({ "chmod", "+x", exe })
  end
  cb(nil, exe)
end

return M

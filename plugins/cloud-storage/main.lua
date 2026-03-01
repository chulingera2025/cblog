-- 计算文件的 SHA-256 哈希（十六进制），用于 S3 签名
local function file_sha256_hex(filepath)
    local content = cblog.files.read(filepath)
    -- 使用简化的哈希标识：对于文件上传场景使用 UNSIGNED-PAYLOAD 更通用
    return "UNSIGNED-PAYLOAD"
end

-- 根据文件扩展名推断 Content-Type
local function guess_content_type(filename)
    local ext = filename:match("%.([^%.]+)$")
    if not ext then return "application/octet-stream" end
    ext = ext:lower()
    local types = {
        jpg = "image/jpeg", jpeg = "image/jpeg", png = "image/png",
        gif = "image/gif", webp = "image/webp", svg = "image/svg+xml",
        ico = "image/x-icon", bmp = "image/bmp", avif = "image/avif",
        mp4 = "video/mp4", webm = "video/webm",
        mp3 = "audio/mpeg", ogg = "audio/ogg", wav = "audio/wav",
        pdf = "application/pdf", json = "application/json",
        css = "text/css", js = "application/javascript",
        html = "text/html", xml = "application/xml",
        zip = "application/zip", gz = "application/gzip",
    }
    return types[ext] or "application/octet-stream"
end

-- 递归扫描目录，返回所有文件的相对路径列表
local function scan_dir(base_path, prefix)
    prefix = prefix or ""
    local results = {}
    local ok, entries = pcall(cblog.files.list, base_path)
    if not ok then return results end

    for _, name in ipairs(entries) do
        local rel = (prefix == "") and name or (prefix .. "/" .. name)
        local full = base_path .. "/" .. name
        local sub_ok, sub_entries = pcall(cblog.files.list, full)
        if sub_ok and #sub_entries > 0 then
            local sub = scan_dir(full, rel)
            for _, s in ipairs(sub) do
                results[#results + 1] = s
            end
        else
            results[#results + 1] = rel
        end
    end
    return results
end

-- 上传单个文件到 S3 兼容存储
local function upload_file(cfg, filepath, object_key)
    local endpoint = cfg.endpoint
    local bucket = cfg.bucket
    local region = cfg.region or "us-east-1"
    local access_key = cfg.access_key
    local secret_key = cfg.secret_key

    local url = string.format("%s/%s/%s", endpoint, bucket, object_key)
    local content_type = guess_content_type(object_key)
    local payload_hash = "UNSIGNED-PAYLOAD"

    local signed_headers = cblog.s3.sign_headers(
        "PUT", url, region, access_key, secret_key,
        { ["content-type"] = content_type },
        payload_hash
    )

    local resp = cblog.http.put_file(url, filepath, { headers = signed_headers })

    if resp.status >= 200 and resp.status < 300 then
        return true
    else
        cblog.log.warn(string.format(
            "上传失败 %s → %s (HTTP %d): %s",
            filepath, object_key, resp.status,
            resp.body:sub(1, 200)
        ))
        return false
    end
end

-- 替换输出 HTML 中的 /media/ 路径
local function replace_media_urls(output_dir, url_prefix, files)
    if not url_prefix or url_prefix == "" then return end

    -- 确保 url_prefix 末尾无斜杠
    url_prefix = url_prefix:gsub("/$", "")

    for _, name in ipairs(files) do
        if name:match("%.html$") then
            local path = output_dir .. "/" .. name
            local ok, content = pcall(cblog.files.read, path)
            if ok and content then
                local new_content = content:gsub("/media/", url_prefix .. "/")
                if new_content ~= content then
                    cblog.files.write(path, new_content)
                end
            end
        end
    end
end

plugin.action("after_finalize", 50, function(ctx)
    local cfg = plugin.config()
    if not cfg or cfg.enabled ~= "true" then
        return
    end

    -- 验证必要配置
    if not cfg.endpoint or cfg.endpoint == "" then
        cblog.log.warn("[cloud-storage] 未配置 endpoint，跳过上传")
        return
    end
    if not cfg.bucket or cfg.bucket == "" then
        cblog.log.warn("[cloud-storage] 未配置 bucket，跳过上传")
        return
    end
    if not cfg.access_key or cfg.access_key == "" then
        cblog.log.warn("[cloud-storage] 未配置 access_key，跳过上传")
        return
    end
    if not cfg.secret_key or cfg.secret_key == "" then
        cblog.log.warn("[cloud-storage] 未配置 secret_key，跳过上传")
        return
    end

    local media_dir = "media"
    if not cblog.files.exists(media_dir) then
        cblog.log.info("[cloud-storage] media 目录不存在，跳过上传")
        return
    end

    cblog.log.info("[cloud-storage] 开始上传 media 文件到云存储...")

    local media_files = scan_dir(media_dir, "")
    local success_count = 0
    local fail_count = 0

    for _, rel_path in ipairs(media_files) do
        local local_path = media_dir .. "/" .. rel_path
        local object_key = "media/" .. rel_path

        if upload_file(cfg, local_path, object_key) then
            success_count = success_count + 1
        else
            fail_count = fail_count + 1
        end
    end

    cblog.log.info(string.format(
        "[cloud-storage] 上传完成: %d 成功, %d 失败 (共 %d 文件)",
        success_count, fail_count, #media_files
    ))

    -- URL 替换
    if cfg.url_prefix and cfg.url_prefix ~= "" then
        local output_dir = ctx.output_dir or "public"
        local output_files = scan_dir(output_dir, "")
        replace_media_urls(output_dir, cfg.url_prefix, output_files)
        cblog.log.info("[cloud-storage] 已替换输出 HTML 中的 media 路径")
    end
end)

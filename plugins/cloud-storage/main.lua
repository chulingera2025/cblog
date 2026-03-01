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

-- 构建后替换输出 HTML 中的 media URL 前缀
plugin.action("after_finalize", 50, function(ctx)
    local cfg = plugin.config()
    if not cfg or cfg.enabled ~= "true" then
        return
    end

    if cfg.url_prefix and cfg.url_prefix ~= "" then
        local output_dir = ctx.output_dir or "public"
        local output_files = scan_dir(output_dir, "")
        replace_media_urls(output_dir, cfg.url_prefix, output_files)
        cblog.log.info("[cloud-storage] 已替换输出 HTML 中的 media 路径")
    end
end)

-- 媒体上传后同步到 S3
plugin.action("after_media_upload", 50, function(ctx)
    local cfg = plugin.config()
    if not cfg or cfg.enabled ~= "true" then return end

    local endpoint = cfg.endpoint or ""
    local bucket = cfg.bucket or ""
    local region = cfg.region or "us-east-1"
    local access_key = cfg.access_key or ""
    local secret_key = cfg.secret_key or ""

    if endpoint == "" or bucket == "" or access_key == "" or secret_key == "" then
        cblog.log.warn("[cloud-storage] 配置不完整，跳过上传")
        return
    end

    -- 上传主文件
    local object_key = ctx.file_path
    local url = string.format("%s/%s/%s", endpoint, bucket, object_key)
    local headers = cblog.s3.sign_headers("PUT", url, region, access_key, secret_key, nil, "UNSIGNED-PAYLOAD")
    local resp = cblog.http.put_file(url, ctx.file_path, { headers = headers })
    if resp.status >= 200 and resp.status < 300 then
        cblog.log.info("[cloud-storage] 上传成功: " .. object_key)
    else
        cblog.log.error("[cloud-storage] 上传失败: " .. object_key .. " status=" .. tostring(resp.status))
    end

    -- 上传缩略图（如果有）
    if ctx.thumb_path and ctx.thumb_path ~= "" then
        local thumb_key = ctx.thumb_path
        local thumb_url = string.format("%s/%s/%s", endpoint, bucket, thumb_key)
        local thumb_headers = cblog.s3.sign_headers("PUT", thumb_url, region, access_key, secret_key, nil, "UNSIGNED-PAYLOAD")
        local thumb_resp = cblog.http.put_file(thumb_url, ctx.thumb_path, { headers = thumb_headers })
        if thumb_resp.status >= 200 and thumb_resp.status < 300 then
            cblog.log.info("[cloud-storage] 缩略图上传成功: " .. thumb_key)
        else
            cblog.log.error("[cloud-storage] 缩略图上传失败: " .. thumb_key .. " status=" .. tostring(thumb_resp.status))
        end
    end
end)

-- 媒体删除后从 S3 删除
plugin.action("after_media_delete", 50, function(ctx)
    local cfg = plugin.config()
    if not cfg or cfg.enabled ~= "true" then return end

    local endpoint = cfg.endpoint or ""
    local bucket = cfg.bucket or ""
    local region = cfg.region or "us-east-1"
    local access_key = cfg.access_key or ""
    local secret_key = cfg.secret_key or ""

    if endpoint == "" or bucket == "" or access_key == "" or secret_key == "" then return end

    -- 从 URL 提取对象路径，ctx.url 格式如 "/media/2026/03/xxx.webp"
    if ctx.url and ctx.url ~= "" then
        local object_key = ctx.url:sub(2)
        local url = string.format("%s/%s/%s", endpoint, bucket, object_key)
        local headers = cblog.s3.sign_headers("DELETE", url, region, access_key, secret_key)
        local resp = cblog.http.delete(url, { headers = headers })
        if resp.status >= 200 and resp.status < 300 then
            cblog.log.info("[cloud-storage] 删除成功: " .. object_key)
        else
            cblog.log.warn("[cloud-storage] 删除失败: " .. object_key .. " status=" .. tostring(resp.status))
        end
    end

    -- 删除缩略图（如果有）
    if ctx.thumb_url and ctx.thumb_url ~= "" then
        local thumb_key = ctx.thumb_url:sub(2)
        local thumb_url_full = string.format("%s/%s/%s", endpoint, bucket, thumb_key)
        local thumb_headers = cblog.s3.sign_headers("DELETE", thumb_url_full, region, access_key, secret_key)
        cblog.http.delete(thumb_url_full, { headers = thumb_headers })
    end
end)

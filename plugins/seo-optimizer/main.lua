plugin.action("after_finalize", 20, function(ctx)
    local site = cblog.site()
    local site_url = site.url or ""

    local content = "User-agent: *\n"
    content = content .. "Allow: /\n"
    content = content .. "Disallow: /admin/\n"
    content = content .. "\n"
    if site_url ~= "" then
        content = content .. "Sitemap: " .. site_url .. "/sitemap.xml\n"
    end

    local output = ctx.output_dir or "public"
    cblog.files.write(output .. "/robots.txt", content)
    cblog.log.info("已生成 robots.txt")
end)

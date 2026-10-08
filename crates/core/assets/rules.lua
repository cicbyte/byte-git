function get_proxies()
    return {
        {
            name = "ghp",
            url = "https://ghfast.top/",
            default = true
        },
        {
            name = "fastgit",
            url = "https://hub.fastgit.xyz/",
            default = false
        }
    }
end

function get_accelerated_url(repo_url, proxy_name)
    local proxies = get_proxies()
    
    -- 查找匹配代理
    for _, proxy in ipairs(proxies) do
        if proxy.name == proxy_name then
            -- 特殊处理 SSH 协议
            if string.find(repo_url, "git@github.com") then
                return string.gsub(repo_url, "git@github.com:", proxy.url)
            end
            return proxy.url .. repo_url
        end
    end
    
    -- 未找到时使用默认代理
    for _, proxy in ipairs(proxies) do
        if proxy.default then
            return proxy.url .. repo_url
        end
    end
    
    error("No available proxy")
end
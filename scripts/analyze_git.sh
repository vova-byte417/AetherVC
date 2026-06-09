#!/bin/bash
cd /mnt/d/trae_projects/AetherVC

echo "=== .git 总大小 ==="
du -sh .git

echo ""
echo "=== 子目录分布 ==="
du -sh .git/*/ 2>/dev/null | sort -rh

echo ""
echo "=== 提交历史 ==="
echo "提交数: $(git rev-list --count --all)"
echo "分支数: $(git branch -a | wc -l)"

echo ""
echo "=== hooks 目录 ==="
ls -la .git/hooks/

echo ""
echo "=== 大对象 (大于10K) ==="
find .git/objects -type f -size +10k -exec ls -lh {} \; 2>/dev/null

echo ""
echo "=== 对象统计 ==="
git count-objects -vH 2>&1

echo ""
echo "========================================="
echo "  清理建议"
echo "========================================="
echo "1. hooks 示例文件 (~48K) -- 删除所有 *.sample"
echo "2. git gc --aggressive    -- 压缩松散对象 908K -> ~150K"
echo "3. target/ 编译产物       -- cargo clean 释放几百MB"

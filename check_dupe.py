#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import sys
from collections import defaultdict

def main():
    if len(sys.argv) != 2:
        print("用法: python script.py <文件名>")
        print("文件每行格式: <词语>\\t<编码>")
        sys.exit(1)

    filename = sys.argv[1]
    code_to_words = defaultdict(list)

    try:
        with open(filename, 'r', encoding='utf-8') as f:
            for line_num, line in enumerate(f, 1):
                line = line.rstrip('\n')
                if not line:
                    continue
                parts = line.split('\t')
                if len(parts) != 2:
                    print(f"警告: 第{line_num}行格式错误，已跳过: {line}")
                    continue
                word, code = parts[0].strip(), parts[1].strip()
                code_to_words[code].append(word)
    except FileNotFoundError:
        print(f"错误: 文件 '{filename}' 不存在")
        sys.exit(1)
    except Exception as e:
        print(f"读取文件时发生错误: {e}")
        sys.exit(1)

    # 筛选出有重码的组（词语数量 >= 2）
    duplicate_groups = {code: words for code, words in code_to_words.items() if len(words) >= 2}

    if not duplicate_groups:
        print("没有发现任何重码词组。")
        return

    # 输出重码词组总数
    total_groups = len(duplicate_groups)
    dupe_percentage = total_groups / len(code_to_words) * 100.0
    print(f"重码词组总数: {total_groups} （总数：{len(code_to_words)}） 占比为：{dupe_percentage:.2f}% ")

    # 找出最大重码数量
    max_dup_count = max(len(words) for words in duplicate_groups.values())
    # 找出所有达到最大重码数量的编码
    max_codes = [code for code, words in duplicate_groups.items() if len(words) == max_dup_count]

    # 输出重码率最高的编码信息
    if len(max_codes) == 1:
        print(f"重码率最高的编码: {max_codes[0]} (重码数: {max_dup_count})")
    else:
        codes_str = ", ".join(max_codes)
        print(f"重码率最高的编码: {codes_str} (重码数: {max_dup_count})")

    candidate_counts = {len(words): [] for code, words in duplicate_groups.items()}
    for code, words in duplicate_groups.items():
        candidate_counts[len(words)].append(code)
    for candidate_count in sorted(candidate_counts.keys(), reverse=True):
        num_codes = len(candidate_counts[candidate_count])
        dupe_percentage = num_codes / len(code_to_words) * 100.0
        codes_str = ("，编码为：" + ", ".join(candidate_counts[candidate_count])) if num_codes <= 5 else ""
        print(f"候选数为{candidate_count}的编码有{num_codes}个，占比为：{dupe_percentage:.2f}%{codes_str}")

    # 打印每个重码词组
    # 按编码排序，使输出更整齐
    for code in sorted(duplicate_groups.keys()):
        words = duplicate_groups[code]
        print(f"{code}：{' '.join(words)}")

if __name__ == "__main__":
    main()

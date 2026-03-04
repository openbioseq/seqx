#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SEQX_BIN="${ROOT_DIR}/target/release/seqx"
WORK_DIR="${ROOT_DIR}/target/bench_packed_io"

N_RECORDS="${N_RECORDS:-200000}"
SEQ_LEN="${SEQ_LEN:-150}"
DUP_RATE="${DUP_RATE:-20}"
SEED="${SEED:-42}"

mkdir -p "${WORK_DIR}"

if [[ ! -x "${SEQX_BIN}" ]]; then
  echo "[INFO] 未找到 release 二进制，先构建..."
  (cd "${ROOT_DIR}" && cargo build --release -q)
fi

INPUT_FA="${WORK_DIR}/input.fa"
SORT_OUT="${WORK_DIR}/sorted.fa"
DEDUP_OUT="${WORK_DIR}/dedup.fa"

echo "[INFO] 生成基准数据: N_RECORDS=${N_RECORDS}, SEQ_LEN=${SEQ_LEN}, DUP_RATE=${DUP_RATE}, SEED=${SEED}"
python3 - <<'PY' "${INPUT_FA}" "${N_RECORDS}" "${SEQ_LEN}" "${DUP_RATE}" "${SEED}"
import random
import sys

out = sys.argv[1]
n = int(sys.argv[2])
seq_len = int(sys.argv[3])
dup_rate = int(sys.argv[4])
seed = int(sys.argv[5])
random.seed(seed)

bases = "ACGT"
uniq_pool = max(1, n * (100 - dup_rate) // 100)
pool = []
for i in range(uniq_pool):
    s = "".join(random.choice(bases) for _ in range(seq_len))
    pool.append((f"seed_{i}", s))

with open(out, "w", encoding="utf-8") as f:
    for i in range(n):
        if random.randint(1, 100) <= dup_rate and pool:
            pid, seq = random.choice(pool)
            rid = f"dup_{i}_{pid}"
        else:
            seq = "".join(random.choice(bases) for _ in range(seq_len))
            rid = f"uniq_{i}"
        f.write(f">{rid}\n{seq}\n")
PY

human_size() {
  local path="$1"
  if [[ -f "$path" ]]; then
    stat -c "%s" "$path" | awk '{
      split("B KB MB GB TB", u, " ");
      s=$1; i=1;
      while (s>=1024 && i<5) { s/=1024; i++ }
      printf "%.2f %s", s, u[i]
    }'
  else
    echo "N/A"
  fi
}

time_cmd() {
  local start end elapsed_ms
  start=$(date +%s%3N)
  "$@"
  end=$(date +%s%3N)
  elapsed_ms=$((end - start))
  echo "${elapsed_ms}"
}

echo "[INFO] 运行 sort 基准..."
SORT_MS=$(time_cmd "${SEQX_BIN}" sort -i "${INPUT_FA}" -o "${SORT_OUT}" --by-name --max-memory 64)

echo "[INFO] 运行 dedup 基准..."
DEDUP_MS=$(time_cmd "${SEQX_BIN}" dedup -i "${INPUT_FA}" -o "${DEDUP_OUT}" --buckets 128)

INPUT_SIZE=$(human_size "${INPUT_FA}")
SORT_SIZE=$(human_size "${SORT_OUT}")
DEDUP_SIZE=$(human_size "${DEDUP_OUT}")

echo
echo "========== packed-seq I/O benchmark =========="
echo "输入文件: ${INPUT_FA} (${INPUT_SIZE})"
echo "输出(sort): ${SORT_OUT} (${SORT_SIZE})"
echo "输出(dedup): ${DEDUP_OUT} (${DEDUP_SIZE})"
echo "sort 耗时:  ${SORT_MS} ms"
echo "dedup 耗时: ${DEDUP_MS} ms"
echo "==============================================="
echo

echo "[NOTE] 如需更大规模测试，可设置环境变量："
echo "  N_RECORDS=1000000 SEQ_LEN=200 DUP_RATE=40 ./scripts/bench_packed_io.sh"

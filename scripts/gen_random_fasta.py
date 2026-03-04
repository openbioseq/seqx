#!/usr/bin/env python3

import argparse
import random


def generate_random_sequence(length: int, alphabet: str, rng: random.Random) -> str:
    return "".join(rng.choices(alphabet, k=length))


def write_fasta(
    output_path: str,
    count: int,
    alphabet: str,
    line_width: int,
    seed: int | None,
) -> None:
    rng = random.Random(seed)

    with open(output_path, "w", encoding="utf-8") as handle:
        for idx in range(1, count + 1):
            handle.write(f">seq{idx}\n")

            length = random.randint(500, 2000) * random.randint(1, 10)

            sequence = generate_random_sequence(length, alphabet, rng)

            if line_width <= 0:
                handle.write(sequence + "\n")
            else:
                for start in range(0, length, line_width):
                    handle.write(sequence[start : start + line_width] + "\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate random FASTA sequences.")
    parser.add_argument(
        "-o", "--output", default="random_1m.fasta", help="Output FASTA file path"
    )
    parser.add_argument(
        "-n", "--num-seqs", type=int, default=1_000_000, help="Number of sequences"
    )
    parser.add_argument(
        "-a", "--alphabet", default="ACGT", help="Alphabet used to sample residues"
    )
    parser.add_argument(
        "--line-width",
        type=int,
        default=60,
        help="FASTA line width (<=0 means single-line sequence)",
    )
    parser.add_argument(
        "--seed", type=int, default=None, help="Random seed for reproducibility"
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()

    if args.num_seqs <= 0:
        raise ValueError("--num-seqs must be > 0")
    if not args.alphabet:
        raise ValueError("--alphabet must be non-empty")

    write_fasta(
        output_path=args.output,
        count=args.num_seqs,
        alphabet=args.alphabet,
        line_width=args.line_width,
        seed=args.seed,
    )


if __name__ == "__main__":
    main()

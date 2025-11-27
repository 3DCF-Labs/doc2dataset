# LLaMA-Factory (ShareGPT) export

```bash
cargo run -p doc2dataset -- run --config doc2dataset.yaml
ls ./datasets/handbook/exports/llama_factory/sharegpt.jsonl
```

Use the generated file directly in LLaMA-Factory by setting `dataset_name` to the path above and `format` to `sharegpt`.

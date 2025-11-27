# Axolotl + RAG

```bash
cargo run -p doc2dataset -- run --config doc2dataset.yaml
ls ./datasets/policies/exports/axolotl/chat.jsonl
ls ./datasets/policies/exports/rag/train.jsonl
```

Point Axolotl at either file depending on whether you fine-tune chat or retrieval-aware models.

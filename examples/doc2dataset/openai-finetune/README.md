# OpenAI finetune quickstart

```bash
# 1. Build the dataset
cargo run -p doc2dataset -- run --config doc2dataset.yaml

# 2. Upload to OpenAI (assumes `OPENAI_API_KEY` is set)
openai files upload --purpose fine-tune --file ./datasets/company/exports/openai/finetune.jsonl

# 3. Kick off a job
openai fine_tuning.jobs.create --training_file <FILE_ID_FROM_STEP_2> --model gpt-4o-mini-2024-07-18
```

`exports/rag/train.jsonl` is also produced if you want retrieval-aware fine-tuning for Axolotl or custom trainers.

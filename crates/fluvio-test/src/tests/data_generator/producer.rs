use fluvio_test_util::test_runner::test_driver::TestDriver;
use fluvio_test_util::test_meta::environment::EnvDetail;
use std::time::SystemTime;
use tracing::debug;
use fluvio::{TopicProducerPool, Offset, RecordKey, TopicProducerConfigBuilder};
use futures::StreamExt;
use std::time::Duration;

use super::MyTestCase;
use crate::tests::TestRecordBuilder;

pub async fn producer(
    mut test_driver: TestDriver,
    option: MyTestCase,
    producer_id: u32,
    run_id: Option<String>,
) {
    // Sync topic is unique per instance of generator
    let sync_topic = if let Some(run_id) = &run_id {
        format!("sync-{run_id}")
    } else {
        "sync".to_string()
    };

    test_driver
        .connect()
        .await
        .expect("Connecting to cluster failed");

    // Create the testing producer

    // TODO: Create a Vec of producers per topic
    let mut producers = Vec::new();

    for topic_id in 0..option.environment.topic {
        let env_opts = option.environment.clone();

        let test_topic_name = if env_opts.topic > 1 {
            format!("{}-{topic_id}", env_opts.base_topic_name())
        } else {
            env_opts.base_topic_name()
        };

        let mut builder = TopicProducerConfigBuilder::default();
        match (
            option.environment.producer_linger,
            option.environment.producer_batch_size,
            option.environment.producer_compression,
        ) {
            (Some(linger), Some(batch), Some(compression)) => builder
                .linger(Duration::from_millis(linger))
                .batch_size(batch)
                .compression(compression),
            (Some(linger), Some(batch), None) => builder
                .linger(Duration::from_millis(linger))
                .batch_size(batch),
            (Some(linger), None, None) => builder.linger(Duration::from_millis(linger)),
            (Some(linger), None, Some(compression)) => builder
                .linger(Duration::from_millis(linger))
                .compression(compression),
            (None, Some(batch), Some(compression)) => {
                builder.batch_size(batch).compression(compression)
            }

            (None, Some(batch), None) => builder.batch_size(batch),
            (None, None, Some(compression)) => builder.compression(compression),
            (None, None, None) => {
                producers.push(test_driver.create_producer(&test_topic_name).await);
                continue;
            }
        };

        let config = builder.build().expect("producer builder");
        producers.push(
            test_driver
                .create_producer_with_config(&test_topic_name, config)
                .await,
        )
    }

    // Create the syncing producer/consumer

    let sync_producer = test_driver.create_producer(&sync_topic).await;
    let mut sync_stream = test_driver
        .get_consumer_with_start(&sync_topic, 0, Offset::from_end(0))
        .await;

    // Let syncing process know this producer is ready
    sync_producer.send(RecordKey::NULL, "ready").await.unwrap();

    println!("{producer_id}: waiting for start");
    while let Some(Ok(record)) = sync_stream.next().await {
        let value_str = record.get_value().as_utf8_lossy_string();
        if value_str.eq("start") {
            println!("Starting producer");
            break;
        }
    }

    let mut records_sent = 0;
    let test_start = SystemTime::now();

    debug!("About to start producer loop");

    if option.environment.timeout != Duration::MAX {
        while test_start.elapsed().unwrap() <= option.environment.timeout {
            for producer in producers.iter() {
                send_record(&option, producer_id, records_sent, &test_driver, producer).await;
                records_sent += 1;
            }
        }
        println!("Timer is up")
    } else {
        loop {
            for producer in producers.iter() {
                send_record(&option, producer_id, records_sent, &test_driver, producer).await;
                records_sent += 1;
            }
        }
    }

    println!("Producer stopped. Time's up!\nRecords sent: {records_sent}",)
}

async fn send_record(
    option: &MyTestCase,
    producer_id: u32,
    records_sent: u32,
    test_driver: &TestDriver,
    producer: &TopicProducerPool,
) {
    let record = generate_record(option.clone(), producer_id, records_sent);
    test_driver
        .send_count(producer, RecordKey::NULL, record)
        .await
        .expect("Producer Send failed");
}

fn generate_record(option: MyTestCase, producer_id: u32, record_id: u32) -> Vec<u8> {
    let record = TestRecordBuilder::new()
        .with_tag(format!("{record_id}"))
        .with_random_data(option.environment.producer_record_size)
        .build();
    let record_json = serde_json::to_string(&record)
        .expect("Convert record to json string failed")
        .as_bytes()
        .to_vec();

    debug!("{:?}", &record);

    if option.option.verbose {
        println!(
            "[producer-{}] record: {:>7} (size {:>5}): CRC: {:>10}",
            producer_id,
            record_id,
            record_json.len(),
            record.crc,
        );
    }
    record_json
}

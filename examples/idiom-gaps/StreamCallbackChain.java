class StreamCallbackChain {
    void score(JavaRDD<Tuple2<Integer, Hit>> readHits, Broadcast<Database> db, boolean divide) {
        JavaRDD<Hit> hits = readHits.map(Tuple2::_2);
        JavaPairRDD<Integer, Score> scores =
                hits.mapPartitionsToPair(iter -> computeScores(iter, db.value(), divide));
        Map<Integer, Score> map =
                new HashMap<>(scores.reduceByKey(Score::add).collectAsMap());
        consume(map);
    }

    JavaPairRDD<Integer, Score> computeScores(Iterator<Hit> iter, Database database, boolean divide) {
        return null;
    }

    void consume(Map<Integer, Score> map) {
    }
}


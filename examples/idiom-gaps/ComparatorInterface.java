class ComparatorInterface implements Comparator<Record> {
    public int compare(Record left, Record right) {
        int byScore = Integer.compare(left.score, right.score);
        if (byScore != 0) {
            return byScore;
        }
        return left.name.compareTo(right.name);
    }

    void sort(List<Record> records) {
        Collections.sort(records, this);
    }
}

public interface ReaderProduct<T extends ReaderObservation> {
    int value();
    void set(int value);
    int score(T observation);
}

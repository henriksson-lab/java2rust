import java.io.File;

import org.usadellab.trimmomatic.fastq.FastqParser;
import org.usadellab.trimmomatic.fastq.FastqRecord;
import org.usadellab.trimmomatic.fastq.FastqSerializer;
import org.usadellab.trimmomatic.trim.CropTrimmer;
import org.usadellab.trimmomatic.trim.LeadingTrimmer;
import org.usadellab.trimmomatic.trim.MinLenTrimmer;
import org.usadellab.trimmomatic.trim.SlidingWindowTrimmer;
import org.usadellab.trimmomatic.trim.TrailingTrimmer;
import org.usadellab.trimmomatic.trim.Trimmer;

/** Stable behavioural transcript from untouched Trimmomatic 0.40 classes. */
public final class TrimmomaticOracle {
    private static FastqRecord record(String sequence, String quality) {
        return new FastqRecord("read", sequence, "comment", quality, 33);
    }

    private static String visible(FastqRecord record) {
        if (record == null) return "<null>";
        return record.getSequence() + "|" + record.getQuality() + "|" + record.getHeadPos();
    }

    private static String pairVisible(Trimmer trimmer, FastqRecord first, FastqRecord second) {
        FastqRecord[] records = trimmer.processRecords(new FastqRecord[] {first, second});
        String route;
        if (records[0] != null && records[1] != null) route = "P";
        else if (records[0] != null) route = "U1";
        else if (records[1] != null) route = "U2";
        else route = "D";
        return route + ":" + visible(records[0]) + ":" + visible(records[1]);
    }

    private static void roundTrip(String inputPath, String outputPath) throws Exception {
        FastqParser parser = new FastqParser(33);
        FastqSerializer serializer = new FastqSerializer();
        int written = 0;

        parser.open(new File(inputPath));
        serializer.open(new File(outputPath));
        while (parser.hasNext()) {
            FastqRecord record = parser.next();
            record = new LeadingTrimmer(20).processRecord(record);
            if (record != null)
                record = new TrailingTrimmer(20).processRecord(record);
            if (record != null)
                record = new MinLenTrimmer(2).processRecord(record);
            if (record != null) {
                serializer.writeRecord(record);
                written++;
            }
        }
        parser.close();
        serializer.close();
        System.out.println("roundtrip=" + written);
    }

    private static void malformed(String inputPath) throws Exception {
        FastqParser parser = new FastqParser(33);
        try {
            parser.open(new File(inputPath));
            System.out.println("malformed=accepted");
        } catch (RuntimeException expected) {
            System.out.println("malformed=RuntimeException");
        }
    }

    public static void main(String[] args) throws Exception {
        FastqRecord lowEnds = record("ACGTAC", "!!II!!");
        System.out.println("leading=" + visible(new LeadingTrimmer(20).processRecord(lowEnds)));
        System.out.println("trailing=" + visible(new TrailingTrimmer(20).processRecord(lowEnds)));
        System.out.println("crop=" + visible(new CropTrimmer(4).processRecord(lowEnds)));
        System.out.println("crop-long=" + visible(new CropTrimmer(20).processRecord(lowEnds)));
        System.out.println("minlen-keep=" + visible(new MinLenTrimmer(6).processRecord(lowEnds)));
        System.out.println("minlen-drop=" + visible(new MinLenTrimmer(7).processRecord(lowEnds)));
        System.out.println("leading-drop=" + visible(new LeadingTrimmer(20).processRecord(record("AC", "!!"))));
        System.out.println("trailing-empty=" + visible(new TrailingTrimmer(20).processRecord(record("", ""))));
        System.out.println("sliding=" + visible(new SlidingWindowTrimmer(2, 20).processRecord(record("ACGTAC", "IIII!!"))));
        System.out.println("sliding-short=" + visible(new SlidingWindowTrimmer(4, 20).processRecord(record("AC", "II"))));

        FastqRecord chained = record("ACGTACGT", "!!IIII!!");
        chained = new LeadingTrimmer(20).processRecord(chained);
        chained = new TrailingTrimmer(20).processRecord(chained);
        chained = new CropTrimmer(3).processRecord(chained);
        chained = new MinLenTrimmer(3).processRecord(chained);
        System.out.println("chain=" + visible(chained));

        System.out.println("pair-keep=" + pairVisible(new MinLenTrimmer(2),
                record("AC", "II"), record("GT", "II")));
        System.out.println("pair-u1=" + pairVisible(new MinLenTrimmer(2),
                record("AC", "II"), record("G", "I")));
        System.out.println("pair-u2=" + pairVisible(new MinLenTrimmer(2),
                record("A", "I"), record("GT", "II")));
        System.out.println("pair-drop=" + pairVisible(new MinLenTrimmer(2),
                record("A", "I"), record("G", "I")));
        System.out.println("pair-leading=" + pairVisible(new LeadingTrimmer(20),
                record("ACGT", "!!II"), record("TGCA", "I!!!")));

        try {
            record("AC", "!");
            System.out.println("mismatch=accepted");
        } catch (RuntimeException expected) {
            System.out.println("mismatch=RuntimeException");
        }

        roundTrip(args[0], args[1]);
        malformed(args[2]);
    }
}

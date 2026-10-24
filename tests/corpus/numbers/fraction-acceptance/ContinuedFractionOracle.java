/** Independent caller, compiled by javac and never translated. */
public final class ContinuedFractionOracle {
    static String units(String text) {
        if (text==null) return "NULL";
        StringBuilder result=new StringBuilder();
        for (int i=0;i<text.length();i++) result.append(String.format("%04x",(int)text.charAt(i)));
        return result.toString();
    }
    public static void main(String[] args) {
        for (int mode=0;mode<6;mode++) {
            for (int defaults=0;defaults<2;defaults++) {
                if (mode==5 && defaults==1) continue;
                for (int point=0;point<2;point++) {
                    for (int precision=0;precision<2;precision++) {
                        for (int limit : new int[]{0,1,32}) {
                            ContinuedFractionExtension value=ContinuedFractionExtension.create(mode);
                            ContinuedFractionExtension alias=value;
                            String label=mode+":"+defaults+":"+point+":"+precision+":"+limit;
                            System.out.println(label+":initial\t"+value.getCalls()+"\t"+value.getTrace());
                            for (int repeat=0;repeat<2;repeat++) {
                                alias.setFactor(repeat+1.0);
                                try {
                                    double output=value.run(point*2.0,precision==0 ? 1e-12 : Double.NaN,limit,defaults==1);
                                    System.out.println(label+":"+repeat+"\t"+String.format("%016x",Double.doubleToRawLongBits(output))+"\t"+alias.getCalls()+"\t"+alias.getTrace());
                                } catch (RuntimeException error) {
                                    System.out.println(label+":"+repeat+"\tERR\t"+error.getClass().getSimpleName()+"\t"+
                                        (error instanceof ArithmeticException)+"\t"+units(error.getMessage())+"\t"+alias.getCalls()+"\t"+alias.getTrace());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

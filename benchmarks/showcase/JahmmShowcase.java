import be.ac.ulg.montefiore.run.jahmm.*;
import java.util.*;
public class JahmmShowcase {
    public static void main(String[] args) {
        int states=Integer.parseInt(args[0]), length=Integer.parseInt(args[1]), batches=Integer.parseInt(args[2]);
        int alphabet=5;
        double[] pi=new double[states]; double sum=0;
        for(int i=0;i<states;i++){pi[i]=i+1;sum+=pi[i];}
        for(int i=0;i<states;i++)pi[i]/=sum;
        double[][] a=new double[states][states];
        ArrayList<Opdf<ObservationInteger>> opdfs=new ArrayList<>();
        for(int i=0;i<states;i++) {
            sum=0;
            for(int j=0;j<states;j++){a[i][j]=1+((i*17+j*13+7)%29)+(i==j?40:0);sum+=a[i][j];}
            for(int j=0;j<states;j++)a[i][j]/=sum;
            double[] p=new double[alphabet];sum=0;
            for(int k=0;k<alphabet;k++){p[k]=1+((i*11+k*7+3)%19);sum+=p[k];}
            for(int k=0;k<alphabet;k++)p[k]/=sum;
            opdfs.add(new OpdfInteger(p));
        }
        Hmm<ObservationInteger> hmm=new Hmm<>(pi,a,opdfs);
        for(int b=0;b<batches;b++) {
            ArrayList<ObservationInteger> observations=new ArrayList<>(length);
            long x=1234567L+b*1009L;
            for(int t=0;t<length;t++){x=(x*1664525L+1013904223L)&0xffffffffL;observations.add(new ObservationInteger((int)((x>>>16)%alphabet)));}
            ViterbiCalculator v=new ViterbiCalculator(observations,hmm);
            ForwardBackwardScaledCalculator f=new ForwardBackwardScaledCalculator(observations,hmm);
            StringBuilder path=new StringBuilder();
            for(int state:v.stateSequence()){if(path.length()>0)path.append(',');path.append(state);}
            System.out.printf(Locale.ROOT,"%d\t%016x\t%016x\t%s%n",b,
                Double.doubleToRawLongBits(v.lnProbability()),Double.doubleToRawLongBits(f.lnProbability()),path);
        }
    }
}

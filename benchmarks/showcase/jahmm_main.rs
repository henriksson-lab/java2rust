use new_java2rust_focused_output::*;
use std::{cell::RefCell, rc::Rc};
type Obs = Rc<RefCell<ObservationInteger>>;
fn main() {
    let args: Vec<usize> = std::env::args().skip(1).map(|x| x.parse().unwrap()).collect();
    let (states,length,batches)=(args[0],args[1],args[2]);
    let alphabet=5;
    let mut pi:Vec<f64>=(0..states).map(|i|(i+1)as f64).collect();
    let mut sum=0.0;for x in &pi{sum+=*x;}for x in &mut pi{*x/=sum;}
    let mut a=ndarray::Array2::from_elem((states,states),0.0);
    let mut opdfs:Vec<Option<Box<dyn Opdf<Obs>>>>=Vec::new();
    for i in 0..states {
        sum=0.0;
        for j in 0..states{a[[i,j]]=(1+((i*17+j*13+7)%29)+if i==j{40}else{0})as f64;sum+=a[[i,j]];}
        for j in 0..states{a[[i,j]]/=sum;}
        let mut p=vec![0.0;alphabet];sum=0.0;
        for k in 0..alphabet{p[k]=(1+((i*11+k*7+3)%19))as f64;sum+=p[k];}
        for x in &mut p{*x/=sum;}
        let opdf=Rc::new(RefCell::new(OpdfInteger::new_double_array(p).unwrap()));
        opdfs.push(Some(opdf.into()));
    }
    let hmm=HmmValue::Hmm(Rc::new(RefCell::new(Hmm::new_double_array_double_array_array_list(&pi,a,opdfs).unwrap())));
    for b in 0..batches {
        let mut observations:Vec<Obs>=Vec::with_capacity(length);
        let mut x=1234567_u64+b as u64*1009;
        for _ in 0..length{x=(x*1664525+1013904223)&0xffff_ffff;observations.push(Rc::new(RefCell::new(ObservationInteger::new(((x>>16)%alphabet as u64)as i32))));}
        let v=ViterbiCalculator::new(&observations,std::clone::Clone::clone(&hmm)).unwrap();
        let f=ForwardBackwardScaledCalculator::new_list_hmm(observations,std::clone::Clone::clone(&hmm)).unwrap();
        let path=v.state_sequence().iter().map(|x|x.to_string()).collect::<Vec<_>>().join(",");
        println!("{b}\t{:016x}\t{:016x}\t{path}",v.ln_probability().to_bits(),f.ln_probability().to_bits());
    }
}

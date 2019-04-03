(function(Platform) {

   describe("oboe performance (real http)", function(){
      
      var oboe =     Platform.isNode 
                  ?  require('../../dist/oboe-node.js') 
                  :  (window.oboe)
                  ;
               
        
      function url( path ){
         if( Platform.isNode ) {
            return 'http://localhost:4567/' + path;
         } else {
            return '/testServer/' + path;
         }
      }   
      
      
      it('is benchmarked with a complex jsonpath', function() {
         var startTime = now();
         var doneFn = jasmine.createSpy('done');
         var callCount = 0;
      
         oboe(url('static/json/oneHundredRecords.json'))
            .node('!.$result..{age name company}', function(){callCount++})
            .done( doneFn );
             
         waitsFor( function(){ return doneFn.calls.length == 1 }, 
            'the computation under test to be performed', 
            5000 )
         
         runs( function(){
            expect(callCount).toBe(100);
            console.log('took ' + (now() - startTime) + 'ms to evaluate a complex ' +
               'expression many times, finding 100 matches');  
         });                
      })
      
      it('is benchmarked with a simple jsonpath', function() {
         var startTime = now();
         var doneFn = jasmine.createSpy('done');
         var callCount = 0;
      
         oboe(url('static/json/oneHundredRecords.json'))
            .node('name', function(){callCount++})
            .done( doneFn );
             
         waitsFor( function(){ return doneFn.calls.length == 1 }, 
            'the computation under test to be performed', 
            5000 )
         
         runs( function(){
            expect(callCount).toBe(100);
            console.log('took ' + (now() - startTime) + 'ms to evaluate a simple ' +
               'expression many times, finding 100 matches');  
         });                
      })   
      
      
      
      function now() {
         return new Date().valueOf()   
      }        
   });  

})(typeof Platform == 'undefined'? require('../libs/platform.js') : Platform)


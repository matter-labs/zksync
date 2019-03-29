
describe("oboe loaded using require", function() {
     
   it('is not on the global namespace by default', function () {

      expect(window.oboe).toBe(undefined)
   })
   
   it('can be loaded using require', function () {

      var doneTest;

      require(['oboe'], function(oboe){
         expect(require('oboe')).toBeOboe()
         doneTest = true;
      });
      
      waitsFor('oboe to load using require', function(){return doneTest});

   })
   
   it('it not on global after being loaded', function () {

      var doneTest;

      require(['oboe'], function(oboe){
         expect(window.oboe).toBe(undefined)
         doneTest = true;
      });
      
      waitsFor('oboe to load using require', function(){return doneTest});

   })      
   
   beforeEach(function(){
      this.addMatchers({
         toBeOboe:function(){
         
            var potentialOboe = this.actual;
            
            return !!(  potentialOboe && 
                        potentialOboe('foo.json').node
                     );          
         }
      })
   });
   
});  




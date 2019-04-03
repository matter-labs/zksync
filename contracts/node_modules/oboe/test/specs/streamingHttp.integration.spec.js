/* Tests the streaming xhr without stubbing anything. Really just a test that 
*  we've got the interface of the in-browser XHR object pinned down  */


describe('streaming xhr integration (real http)', function() {
   "use strict";
 
   var emittedEvents = [HTTP_START, STREAM_DATA, STREAM_END, FAIL_EVENT, ABORTING];
 
   it('completes',  function() {
     
      // in practice, since we're running on an internal network and this is a small file,
      // we'll probably only get one callback
      var oboeBus = fakePubSub(emittedEvents)         
      streamingHttp(                         
         oboeBus,
         httpTransport(),
         'GET', 
         '/testServer/static/json/smallestPossible.json',
         null // this is a GET, no data to send
      ); 
      
      waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus)
   })
 
   it('can ajax in a small known file',  function() {
     
      // in practice, since we're running on an internal network and this is a small file,
      // we'll probably only get one callback
      var oboeBus = fakePubSub(emittedEvents)         
      streamingHttp(                         
         oboeBus,
         httpTransport(),
         'GET', 
         '/testServer/static/json/smallestPossible.json',
         null // this is a GET, no data to send
      ); 
      
      waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus);            

      runs(function(){
         expect(oboeBus).toHaveGivenStreamEventsInCorrectOrder()
         expect(streamedContentPassedTo(oboeBus)).toParseTo({}) 
      });  
   })
   
   it('fires HTTP_START with status and headers',  function() {
     
      var oboeBus = fakePubSub(emittedEvents)               
      streamingHttp(                         
         oboeBus,
         httpTransport(),
         'GET', 
         '/testServer/echoBackHeadersAsHeaders',
         null, // this is a GET, no data to send
         {'specialheader':'specialValue'}
      ); 
      
      waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus);            

      runs(function(){
         expect(oboeBus(HTTP_START).emit)
            .toHaveBeenCalledWith(
               200,
               headerObjectContaining('specialheader', 'specialValue')
            );       
      });  
   })
   
   it('gives XHR header so server knows this is an xhr request',  function() {
                 
      var oboeBus = fakePubSub(emittedEvents)               
      streamingHttp(                         
         oboeBus,
         httpTransport(),
         'GET', 
         '/testServer/echoBackHeadersAsHeaders'
      ); 
      
      waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus);            

      runs(function(){
         expect(oboeBus(HTTP_START).emit)
            .toHaveBeenCalledWith(
               200,
               headerObjectContaining('X-Requested-With', 'XMLHttpRequest')
            );  
      });
          
   })   
   
   it('fires HTTP_START, STREAM_DATA and STREAM_END in correct order',  function() {
     
      // in practice, since we're running on an internal network and this is a small file,
      // we'll probably only get one callback         
      var oboeBus = fakePubSub(emittedEvents)      
      streamingHttp(                         
         oboeBus,
         httpTransport(),
         'GET', 
         '/testServer/echoBackHeadersAsHeaders',
         null, // this is a GET, no data to send
         {'specialheader':'specialValue'}
      ); 
      
      waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus);            

      runs(function(){
         expect(oboeBus).toHaveGivenStreamEventsInCorrectOrder()
      });            
   })

   it('fires FAIL_EVENT if url does not exist', function () {
         
      var oboeBus = fakePubSub(emittedEvents)
      streamingHttp(
         oboeBus,
         httpTransport(),
         'GET',
         '/testServer/noSuchUrl',
         null
      );

      waitUntil(FAIL_EVENT).isFiredOn(oboeBus);

   })

   it('can ajax in a very large file without missing any',  function() {
   
  
      // in practice, since we're running on an internal network and this is a small file,
      // we'll probably only get one callback
      var oboeBus = fakePubSub(emittedEvents)               
      streamingHttp(                         
         oboeBus,
         httpTransport(),         
         'GET', 
         '/testServer/static/json/twentyThousandRecords.json',
         null // this is a GET, no data to send      
      );
      
      waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus);            

      runs(function(){
         var parsedResult;
      
         expect(function(){

            parsedResult = JSON.parse(streamedContentPassedTo(oboeBus));
            
         }).not.toThrow();

         // as per the name, should have 20,000 records in that file:                     
         expect(parsedResult.result.length).toEqual(20000);
      });  
   })
   
   it('can ajax in a streaming file without missing any',  function() {
   
   
      // in practice, since we're running on an internal network and this is a small file,
      // we'll probably only get one callback         
      var oboeBus = fakePubSub(emittedEvents)      
      streamingHttp(                       
         oboeBus,
         httpTransport(),         
         'GET', 
         '/testServer/tenSlowNumbers?withoutMissingAny',
          null // this is a GET, no data to send      
      );

      waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus);            

      runs(function(){ 
         // as per the name, should have ten numbers in that file:         
         expect(streamedContentPassedTo(oboeBus)).toParseTo([0,1,2,3,4,5,6,7,8,9]);
         expect(oboeBus).toHaveGivenStreamEventsInCorrectOrder()         
      });              
   })

   it('sends cookies by default',  function() {
      
      document.cookie = "token=123456; path=/";

      // in practice, since we're running on an internal network and this is a small file,
      // we'll probably only get one callback         
      var oboeBus = fakePubSub(emittedEvents)
      streamingHttp(
         oboeBus,
         httpTransport(),
         'GET',
         '/testServer/echoBackHeadersAsBodyJson',
         null
      );

      waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus);

      runs(function(){
         var parsedResult = JSON.parse(streamedContentPassedTo(oboeBus));
         expect(parsedResult.cookie).toMatch('token=123456');
      });

   })

   it('does not send cookies by default to cross-domain requests',  function() {
      
      document.cookie = "deniedToken=123456; path=/";

      // in practice, since we're running on an internal network and this is a small file,
      // we'll probably only get one callback
      var oboeBus = fakePubSub(emittedEvents)
      streamingHttp(
         oboeBus,
         httpTransport(),
         'GET',
         crossDomainUrl('/echoBackHeadersAsBodyJson'),
         null
      );

      waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus);

      runs(function(){
         var parsedResult = JSON.parse(streamedContentPassedTo(oboeBus));
         expect(parsedResult.cookie).not.toMatch('deniedToken=123456');
      });
   })

   it('sends cookies to cross-domain requests if withCredentials is true',  function() {

      document.cookie = "corsToken=123456; path=/";

      // in practice, since we're running on an internal network and this is a small file,
      // we'll probably only get one callback
      var oboeBus = fakePubSub(emittedEvents)
      streamingHttp(
         oboeBus,
         httpTransport(),
         'GET',
         crossDomainUrl('/echoBackHeadersAsBodyJson'),
         null, // data
         null, // headers
         true  // withCredentials
      );

      waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus);

      runs(function(){
         var parsedResult = JSON.parse(streamedContentPassedTo(oboeBus));
         expect(parsedResult.cookie).toMatch('corsToken=123456');
      });
   })   
   
   it('can make a post request',  function() {
   
      var payload = {'thisWill':'bePosted','andShould':'be','echoed':'back'};
   
      // in practice, since we're running on an internal network and this is a small file,
      // we'll probably only get one callback         
      var oboeBus = fakePubSub(emittedEvents)      
      streamingHttp(                        
         oboeBus,
         httpTransport(),         
         'POST',
         '/testServer/echoBackBody',
         JSON.stringify(payload)       
      );

      waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus);            
 
      runs(function(){
         expect(streamedContentPassedTo(oboeBus)).toParseTo(payload);
         expect(oboeBus).toHaveGivenStreamEventsInCorrectOrder()         
      });
     
   })
   
   it('can make a put request',  function() {
   
      var payload = {'thisWill':'bePut','andShould':'be','echoed':'back'};
   
      // in practice, since we're running on an internal network and this is a small file,
      // we'll probably only get one callback         
      var oboeBus = fakePubSub(emittedEvents)      
      streamingHttp(
         oboeBus,
         httpTransport(),         
         'PUT',
         '/testServer/echoBackBody',
         JSON.stringify(payload)       
      );

      waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus);            

      runs(function(){
         expect(streamedContentPassedTo(oboeBus)).toParseTo(payload);
         expect(oboeBus).toHaveGivenStreamEventsInCorrectOrder()         
      });
     
   }) 

  
   it('can make a patch request',  function() {
   
      if( Platform.isInternetExplorer ) {
         console.warn('PATCH requests don\'t work well under IE. Skipping PATCH integration test');
         return;
      }
      
      var payload = {'thisWill':'bePatched','andShould':'be','echoed':'back'};
   
      // in practice, since we're running on an internal network and this is a small file,
      // we'll probably only get one callback         
      var oboeBus = fakePubSub(emittedEvents)      
      streamingHttp(
         oboeBus,
         httpTransport(),         
         'PATCH',
         '/testServer/echoBackBody',
         JSON.stringify(payload)       
      );
      
      waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus);

      runs(function(){
         if( streamedContentPassedTo(oboeBus) == '' &&
             (Platform.isPhantom) ) {
            console.warn( 'this user agent seems not to support giving content' 
                          + ' back for of PATCH requests.'
                          + ' This happens on PhantomJS');
         } else {         
            expect(streamedContentPassedTo(oboeBus)).toParseTo(payload);
            expect(oboeBus).toHaveGivenStreamEventsInCorrectOrder();
         }            
      });
     
   })
   
          
   // this test is only activated for non-IE browsers and IE 10 or newer.
   // old and rubbish browsers buffer the xhr response meaning that this 
   // will never pass. But for good browsers it is good to have an integration
   // test to confirm that we're getting it right.           
   if( !Platform.isInternetExplorer || Platform.isInternetExplorer >= 10 ) {          
      it('gives multiple callbacks when loading a streaming resource',  function() {
                              
         var oboeBus = fakePubSub(emittedEvents)
         streamingHttp(                           
            oboeBus,
            httpTransport(),            
            'GET',

            '/testServer/tenSlowNumbers',
             null // this is a get: no data to send         
         );                     
         
         waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus);      
   
         runs(function(){
                                   
            // realistically, should have had 10 or 20, but this isn't deterministic so
            // 3 is enough to indicate the results didn't all arrive in one big blob.
            expect(oboeBus.callCount[STREAM_DATA]).toBeGreaterThan(3)
            expect(oboeBus).toHaveGivenStreamEventsInCorrectOrder()            
         });      
      })
                     
      it('gives multiple callbacks when loading a gzipped streaming resource',  function() {
                              
         var oboeBus = fakePubSub(emittedEvents)                              
         streamingHttp(                           
            oboeBus,
            httpTransport(),            
            'GET',
 
            '/testServer/gzippedTwoHundredItems',
             null // this is a get: no data to send         
         );                     
         
         waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus);      
   
         runs(function(){
            // some platforms can't help but not work here so warn but don't
            // fail the test:
            if( oboeBus.callCount[STREAM_DATA] == 1 && 
                  (Platform.isInternetExplorer || Platform.isPhantom) ) {
               console.warn('This user agent seems to give gzipped responses' +
                   'as a single event, not progressively. This happens on ' +
                   'PhantomJS and IE < 9');
            } else {
               expect(oboeBus.callCount[STREAM_DATA]).toBeGreaterThan(1);
            }
         
            expect(oboeBus).toHaveGivenStreamEventsInCorrectOrder();
         });      
      })      
   }
   
   it('does not call back with zero-length bites',  function() {
                         
      // since this is a large file, even serving locally we're going to get multiple callbacks:       
      var oboeBus = fakePubSub(emittedEvents)      
      streamingHttp(              
         oboeBus,
         httpTransport(),         
         'GET', 
         '/testServer/static/json/oneHundredRecords.json',
         null // this is a GET: no data to send      
      );         

      waitUntil(STREAM_END, 'the stream to end').isFiredOn(oboeBus)
      
      runs(function(){
      
         var dripsReceived = oboeBus.eventTypesEmitted[STREAM_DATA].map(function( args ){
            return args[0];
         });
      
         expect(dripsReceived.length).toBeGreaterThan(0);
      
         dripsReceived.forEach(function(drip) {            
            expect(drip.length).not.toEqual(0);                                                                                     
         });
      
      })   
   })

   function waitUntil(event, messageName) {
      return {isFiredOn: function (eventBus){
         waitsFor(function(){
        
            return !!eventBus(event).emit.calls.length;
        
            }, 'event ' + event + (messageName?'('+messageName+')':'') + ' to be fired', ASYNC_TEST_TIMEOUT);
         }
      }
   }    
       
   function streamedContentPassedTo(eventBus){
   
      return eventBus.eventTypesEmitted[STREAM_DATA].map(function(args){
         return args[0];
      }).join('');      
   }
   
   beforeEach(function(){
               
      this.addMatchers({
         toHaveGivenStreamEventsInCorrectOrder: function(){
            
            var eventNames = this.actual.eventNames;
                        
            this.message = function(){
               return 'events not in correct order. We have: ' +
                        JSON.stringify(
                           eventNames.map(prettyPrintEvent)
                        ) + ' but should follow "start", "data"*, "end"'
            };
            
            return   eventNames[0] === HTTP_START
                  && eventNames[1] === STREAM_DATA
                  && eventNames[eventNames.length-1] === STREAM_END;            
         },
      
         toParseTo:function( expectedObj ){

            var actual = this.actual;
            var normalisedActual;
                       
            if( !actual ) {
               this.message = function(){
                  return 'no content has been received';
               }
               return false;
            }                       
                       
            try{
               normalisedActual = JSON.stringify( JSON.parse(actual) );
            }catch(e){
            
               this.message = function(){
                
                  return "Expected to be able to parse the found " +
                      "content as json '" + actual + "' but it " +
                      "could not be parsed";                  
               }
               
               return false;          
            }   
            
            this.message = function(){
               return "The found json parsed but did not match " + JSON.stringify(expectedObj) + 
                        " because found " + this.actual; 
            }
                        
            return (normalisedActual === JSON.stringify(expectedObj));
         }
      });
      

      
   });
   
   
   function headerObjectContaining(key, val) {
      // some browsers lowercase the header keys. Compare upper and lower
      // case versions:
   
      return {
         jasmineMatches: function(obj){
            return obj[key] == val || obj[key.toLowerCase()] == val;
         }
      }
   }   

});

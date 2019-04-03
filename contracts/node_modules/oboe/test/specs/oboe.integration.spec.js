
(function(Platform) {

    // Used to spy on global functions like setTimeout
    var globalContext;
    if ( !Platform.isNode ) {
      globalContext = window;
    } else {
      globalContext = GLOBAL;
    }

   describe("oboe integration (real http)", function() {

      var oboe =     Platform.isNode
                  ?  require('../../dist/oboe-node.js')
                  :  (window.oboe)
                  ;

      var testUrl = Platform.isNode
            ?  require('../libs/testUrl.js')
            :  (window.testUrl)
            ;

      var ASYNC_TEST_TIMEOUT = 15 * 1000; // 15 seconds

      describe('advertising the url on the Oboe instance', function() {

         var exampleUrl = testUrl('echoBackHeadersAsBodyJson');

         it('works after instatiation', function() {

            var instance = oboe({
               url: exampleUrl
            });

            expect(instance.source).toBe(exampleUrl);
         });

         it('works from callbacks', function() {

            var urlOnDone;

            oboe({
               url: exampleUrl
            }).done(function() {
               urlOnDone = this.source;
            });

            waitsFor(function() {
               return urlOnDone
            }, 'the request to have called done', ASYNC_TEST_TIMEOUT);

            runs(function() {
               expect(urlOnDone).toBe(exampleUrl);
            });
         });
      });

      Platform.isNode && describe('when running under node', function(){

         var http = require('http'),
             fs = require('fs'),
             request = require('request');

         it('can read from a stream that is passed in', function() {

            http.request( 'http://localhost:4567/tenSlowNumbers' )
               .on('response', function( res ) {

                  oboe(res)
                  .node('![*]', callbackSpy)
                  .done(whenDoneFn);

               }).on('error', function(e) {

                  console.log(e);
               }).end();


            waitsFor(doneCalled, 'the request to have called done', ASYNC_TEST_TIMEOUT);

            runs(function () {

               expect( callbackSpy.calls.length ).toBe(10);
            });
         });

         it('can read from a stream from Request - https://github.com/jimhigson/oboe.js/issues/65', function() {

            oboe(request('http://localhost:4567/tenSlowNumbers'))
               .node('![*]', callbackSpy)
               .done(whenDoneFn)
               .on('error', function(e) {
                  console.log(e);
               });

            waitsFor(doneCalled, 'the request to have called done', ASYNC_TEST_TIMEOUT);

            runs(function () {

               expect( callbackSpy.calls.length ).toBe(10);
            });
         });


         it('can read from a local file', function() {

            var fileStream = fs.createReadStream('test/json/firstTenNaturalNumbers.json');

            oboe(fileStream)
               .node('![*]', callbackSpy)
               .done(whenDoneFn);

            waitsFor(doneCalled, 'the request to have called done', ASYNC_TEST_TIMEOUT);

            runs(function () {

               expect( callbackSpy.calls.length ).toBe(10);
            });
         });

         it('can read an empty key', function() {

            var fileStream = fs.createReadStream('test/json/emptyKey.json');

            oboe(fileStream)
                .node('!.*', callbackSpy)
                .done(whenDoneFn);

            waitsFor(doneCalled, 'the request to have called done', ASYNC_TEST_TIMEOUT);

            runs(function () {

               expect( callbackSpy.calls.length ).toBe(2);
            });
         });

         it('doesnt get confused if a stream has a "url" property', function() {
            var fileStream = fs.createReadStream('test/json/firstTenNaturalNumbers.json');
            fileStream.url = 'http://howodd.com';

            oboe(fileStream)
               .done(whenDoneFn);

            waitsFor(doneCalled, 'the request to have called done', ASYNC_TEST_TIMEOUT);
         });


         it('can read from https', function() {

            // rather than set up a https server in the tests
            // let's just use npm since this is an integration test
            // by confirming the Oboe.js homepage...

            oboe({
               url: 'https://registry.npmjs.org/oboe'
            })
               .node('!.homepage', callbackSpy)
               .done(whenDoneFn);


            waitsFor(doneCalled, 'the request to have called done', ASYNC_TEST_TIMEOUT);

            runs(function () {
               var oboeHomepage = 'http://oboejs.com';
               expect(callbackSpy.mostRecentCall.args[0]).toEqual(oboeHomepage);
            });

         });

         it('complains if given a non-http(s) url but is happy otherwise', function() {

            expect(function() {
               oboe('ftp://ftp.mozilla.org/pub/mozilla.org/')
            }).toThrow();

            expect(function() {
               oboe('http://registry.npmjs.org/oboe')
            }).not.toThrow();

            expect(function() {
               oboe('https://registry.npmjs.org/oboe')
            }).not.toThrow();
         });

      });

      (!Platform.isNode) && describe('running under a browser', function(){
         it('does not send cookies cross-domain by default', function () {

            document.cookie = "oboeIntegrationDontSend=123; path=/";

            var req = oboe({
               url: crossDomainUrl('/echoBackHeadersAsBodyJson')
            }).done(whenDoneFn);

            waitsFor(doneCalled, 'the request to have called done', ASYNC_TEST_TIMEOUT);

            runs(function(){
               expect( req.root().cookie ).toBeUndefined();
            })
         });

         it('can send cookies cross-domain', function () {
            document.cookie = "oboeIntegrationSend=123; path=/";

            var req = oboe({
               url: crossDomainUrl('/echoBackHeadersAsBodyJson'),
               withCredentials: true
            }).done(whenDoneFn);

            waitsFor(doneCalled, 'the request to have called done', ASYNC_TEST_TIMEOUT);

            runs(function(){
               expect( req.root().cookie ).toMatch('oboeIntegrationSend=123');
            })
         });

         it('adds X-Requested-With to cross-domain by default on cross-domain', function () {

            var req = oboe({
               url: testUrl('echoBackHeadersAsBodyJson')
            }).done(whenDoneFn);

            waitsFor(doneCalled, 'the request to have called done', ASYNC_TEST_TIMEOUT);

            runs(function(){
               expect( req.root()['x-requested-with'] ).toEqual('XMLHttpRequest');
            })
         });

         it('does not add X-Requested-With to cross-domain by default on cross-domain', function () {

            var req = oboe({
               url: crossDomainUrl('/echoBackHeadersAsBodyJson')
            }).done(whenDoneFn);

            waitsFor(doneCalled, 'the request to have called done', ASYNC_TEST_TIMEOUT);

            runs(function(){
               expect( req.root()['x-requested-with'] ).toBeUndefined();
            })
         });
      });

      it('gets all expected callbacks by time request finishes', function () {

         oboe(testUrl('tenSlowNumbers'))
             .node('![*]', callbackSpy)
             .done(whenDoneFn);

         waitsFor(doneCalled, 'the request to have called done', ASYNC_TEST_TIMEOUT);

         runs(function () {

            expect(callbackSpy).toHaveBeenCalledWith(0, [0], [fullResponse, 0]);
            expect(callbackSpy).toHaveBeenCalledWith(1, [1], [fullResponse, 1]);
            expect(callbackSpy).toHaveBeenCalledWith(2, [2], [fullResponse, 2]);
            expect(callbackSpy).toHaveBeenCalledWith(3, [3], [fullResponse, 3]);
            expect(callbackSpy).toHaveBeenCalledWith(4, [4], [fullResponse, 4]);
            expect(callbackSpy).toHaveBeenCalledWith(5, [5], [fullResponse, 5]);
            expect(callbackSpy).toHaveBeenCalledWith(6, [6], [fullResponse, 6]);
            expect(callbackSpy).toHaveBeenCalledWith(7, [7], [fullResponse, 7]);
            expect(callbackSpy).toHaveBeenCalledWith(8, [8], [fullResponse, 8]);
            expect(callbackSpy).toHaveBeenCalledWith(9, [9], [fullResponse, 9]);

         });
      })


      it('can make nested requests from arrays', function () {

         oboe(testUrl('tenSlowNumbers'))
            .node('![*]', function(outerNumber){

               oboe(testUrl('tenSlowNumbers'))
                  .node('![*]', function(innerNumber){
                     callbackSpy();
                  });
            })
            .done(whenDoneFn);

         waitsFor(
            function () {
               return callbackSpy.calls.length == 100;
            },
            '100 callbacks',
            30 * 1000 // makes a lot of requests so give it a while to complete
         );
      })


      it('continues to parse after a callback throws an exception', function () {

         spyOn(globalContext, 'setTimeout');
         oboe(testUrl('static/json/tenRecords.json'))
            .node('{id name}', function(){

               callbackSpy()
               throw new Error('uh oh!');
            })
            .done(whenDoneFn);

         waitsFor(
            function () {
               return callbackSpy.calls.length == 10;
            },
            '100 callbacks'
         );

         runs(function () {
            expect(globalContext.setTimeout.calls.length).toEqual(10);
            for(var i = 0; i < 10; i++) {
              expect(globalContext.setTimeout.calls[i].args[0]).toThrow('uh oh!')
            }
         });
      })

      it('can abort once some data has been found in streamed response', function () {

         var req = oboe(testUrl('tenSlowNumbers'))
                     .node('![5]', abortCallback);

         waitsFor(theRequestToBeAborted, 'the request to be aborted', ASYNC_TEST_TIMEOUT);

         // in case we didn't abort, wait a little longer. If we didn't really abort we'd get the
         // rest of the data now and the test would fail:
         waitsFor(someSecondsToPass(2), ASYNC_TEST_TIMEOUT);

         runs(function () {
            // because the request was aborted on index array 5, we got 6 numbers (inc zero)
            // not the whole ten.

            expect(req.root()).toEqual([0, 1, 2, 3, 4, 5]);

         });
      })

      it('can deregister from inside the callback', function () {

         var nodeCallback = jasmine.createSpy().andCallFake(function(){
            if( nodeCallback.calls.length == 5 ) {
               this.forget();
            }
         });

         oboe(testUrl('tenSlowNumbers'))
            .node('![*]', nodeCallback)
            .done(whenDoneFn);

         // in case we didn't abort, wait a little longer. If we didn't really abort we'd get the
         // rest of the data now and the test would fail:
         waitsFor(doneCalled, ASYNC_TEST_TIMEOUT);

         runs(function () {
            // because the request was aborted on index array 5, we got 6 numbers (inc zero)
            // not the whole ten.

            expect(nodeCallback.calls.length).toBe(5);

         });
      })

      it('can still gets the whole resource after deregistering the callback', function () {

         var callback = jasmine.createSpy().andCallFake(function(){
            if( callback.calls.length == 5 ) {
               this.forget();
            }
         });

         oboe(testUrl('tenSlowNumbers'))
            .node('![*]', callback)
            .done(whenDoneFn);

         // in case we didn't abort, wait a little longer. If we didn't really abort we'd get the
         // rest of the data now and the test would fail:
         waitsFor(doneCalled, ASYNC_TEST_TIMEOUT);

         runs(function () {
            // because the request was aborted on index array 5, we got 6 numbers (inc zero)
            // not the whole ten.

            expect(fullResponse).toEqual([0,1,2,3,4,5,6,7,8,9]);
         });
      })

      it('can send query params', function () {

         var apiKeyCallback = jasmine.createSpy();

         oboe(testUrl('echoBackQueryParamsAsBodyJson?apiKey=123'))
            .node('apiKey', apiKeyCallback)
            .done(whenDoneFn);

         waitsFor(doneCalled, ASYNC_TEST_TIMEOUT);

         runs(function () {
            expect(apiKeyCallback.mostRecentCall.args[0]).toBe('123');
         });
      })

      it('can abort once some data has been found in not very streamed response', function () {

         // like above but we're getting a static file not the streamed numbers. This means
         // we'll almost certainly read in the whole response as one onprogress it is on localhost
         // and the json is very small

         var req = oboe(testUrl('static/json/firstTenNaturalNumbers.json'))
                     .node('![5]', abortCallback);

         waitsFor(theRequestToBeAborted, 'the request to be aborted', ASYNC_TEST_TIMEOUT);

         // in case we didn't abort, wait a little longer. If we didn't really abort we'd get the
         // rest of the data now and the test would fail:
         waitsFor(someSecondsToPass(2), ASYNC_TEST_TIMEOUT);

         runs(function () {
            // because the request was aborted on index array 5, we got 6 numbers (inc zero)
            // not the whole ten.

            expect(req.root()).toEqual([0, 1, 2, 3, 4, 5]);
         });
      })

      it('gives full json to callback when request finishes', function () {

         oboe(testUrl('static/json/firstTenNaturalNumbers.json'))
             .done(whenDoneFn);

         waitsFor(doneCalled, 'the request to give full response', ASYNC_TEST_TIMEOUT)

         runs(function () {
            expect(fullResponse).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8, 9])
         });
      })

      it('gives content type as JSON when provided with an Object body', function () {

         var done;

         oboe({
            method:'PUT',
            url:testUrl('echoBackHeadersAsBodyJson'),
            body:{'potatoes':3, 'cabbages':4}

         }).done(function(json){
            done = true;
            var contentType = contextTypeFrom(json['content-type']);
            expect(contentType).toBe('application/json');
         });

         waitsFor(function (){ return !!done; }, 'the request to complete', ASYNC_TEST_TIMEOUT)
      })

      it('gives allows content type to be overridden when provided with an Object body', function () {

          var done;

          oboe({
             method:'PUT',
             url:testUrl('echoBackHeadersAsBodyJson'),
             body:{'potatoes':3, 'cabbages':4},
             headers:{'Content-Type':'application/vegetableDiffThing'}

          }).done(function(json){
             done = true;
             var contentType = contextTypeFrom(json['content-type']);
             expect(contentType).toBe('application/vegetableDiffThing');
          });

          waitsFor(function (){ return !!done; }, 'the request to complete', ASYNC_TEST_TIMEOUT)
      })

      it('gives header to server side', function () {

         var countGotBack = 0;

         oboe(
             {  url:testUrl('echoBackHeadersAsBodyJson'),
                headers:{'x-snarfu':'SNARF', 'x-foo':'BAR'}
             }

         ).node('x-snarfu', function (headerValue) {

            expect(headerValue).toBe('SNARF')
            countGotBack++;

         }).node('x-foo', function (headerValue) {

            expect(headerValue).toBe('BAR')
            countGotBack++;
         })

         waitsFor(function () {
            return countGotBack == 2
         }, 'all headers to have been detected back here', ASYNC_TEST_TIMEOUT)
      })

      it('can read response headers', function () {

         var done = false;

         oboe(
             {  url:testUrl('echoBackHeadersAsHeaders'),
                headers:{'x-sso':'sso', 'x-sso2':'sso2'}
             }

         ).done(function(){

            expect(this.header('x-sso')).toEqual('sso');
            expect(this.header('x-sso2')).toEqual('sso2');

            expect(this.header()['x-sso']).toEqual('sso');
            expect(this.header()['x-sso2']).toEqual('sso2');
            expect(this.header()['x-sso3']).toBeUndefined();

            done = true;
         })

         waitsFor(function () {
            return done;
         }, 'the response to complete', ASYNC_TEST_TIMEOUT)
      })

      it('gives undefined for headers before they are ready', function () {

         var o = oboe(
             {  url:testUrl('echoBackHeadersAsHeaders'),
                headers:{'x-sso':'sso', 'x-sso2':'sso2'}
             }
         )

         expect(o.header()).toBeUndefined();
         expect(o.header('x-sso')).toBeUndefined();
         expect(o.header('x-sso2')).toBeUndefined();
      })

      it('notifies of response starting by giving status code and headers to callback', function () {
         var called = 0;

         oboe(
             {  url:testUrl('echoBackHeadersAsHeaders'),
                headers:{'x-a':'A', 'x-b':'B'}
             }
         ).start(function(statusCode, headers){

            expect(statusCode).toBe(200);
            expect(headers['x-a']).toBe('A');
            called++;

         }).on('start', function(statusCode, headers){

            expect(statusCode).toBe(200);
            expect(headers['x-b']).toBe('B');
            called++;
         });

         waitsFor(function () {
            return called == 2;
         }, 'the response to call both start callbacks', ASYNC_TEST_TIMEOUT)
      });

      it('can listen for nodes nodejs style', function () {

         var countGotBack = 0;

         oboe(testUrl('static/json/firstTenNaturalNumbers.json'))
            .on('node', '!.*', function () {
               countGotBack++;
            });

         waitsFor(function () {
            return countGotBack == 10
         }, 'ten callbacks', ASYNC_TEST_TIMEOUT)
      })

      it('can listen for paths nodejs style', function () {

         var countGotBack = 0;

         oboe(testUrl('static/json/firstTenNaturalNumbers.json'))

            .on('path', '!.*', function (number) {
               countGotBack++;
            });

         waitsFor(function () {
            return countGotBack == 10
         }, 'ten callbacks', ASYNC_TEST_TIMEOUT)
      })

      it('can listen for done nodejs style', function () {

         oboe(testUrl('static/json/firstTenNaturalNumbers.json'))

            .on('done', whenDoneFn);

         waitsFor(doneCalled, 'the request to finish', ASYNC_TEST_TIMEOUT)
      })

      it('gets all callbacks and they are in correct order', function () {
         var order = [];

         oboe({
            method:'POST',
            url: testUrl('echoBackBody')
         ,  body: {a:'A', b:'B', c:'C'}
         })
         .path('!', function(){ order.push(1) })
         .path('a', function(){ order.push(2) })
         .node('a', function(){ order.push(3) })
         .path('b', function(){ order.push(4) })
         .node('b', function(){ order.push(5) })
         .path('c', function(){ order.push(6) })
         .node('c', function(){ order.push(7) })
         .done(function(){      order.push(8) })

         waitsFor(function(){ return order.length == 8 },
            'all 8 callbacks', ASYNC_TEST_TIMEOUT);

         runs(function(){
            expect(order).toEqual([1,2,3,4,5,6,7,8]);
         });

      });

      it('emits error on 404', function () {

         var stubCallback = jasmine.createSpy('error callback');

         oboe(testUrl('404json'))
            .fail(stubCallback);

         waitsFor(function () {
            return !!stubCallback.calls.length;
         }, 'the request to fail', ASYNC_TEST_TIMEOUT)

         runs( function() {
            expect( stubCallback ).toHaveBeenGivenErrorStatusCode( 404 );
            expect( stubCallback ).toHaveBeenGivenBodyJson(
               {  "found":"false",
                  "errorMessage":"was not found"
               }
            );
         });
      })

      it('emits error on 404 in nodejs style too', function () {

         var stubCallback = jasmine.createSpy('error callback');

         oboe(testUrl('doesNotExist'))
            .on('fail', stubCallback);

         waitsFor(function () {
            return !!stubCallback.calls.length;
         }, 'the request to fail', ASYNC_TEST_TIMEOUT)
      })

      /*
      This isn't reliable enough, too many false negatives for a ci
      it('emits error on unreachable url', function () {

         var stubCallback = jasmine.createSpy('error callback');

         oboe('nowhere.ox.ac.uk:754196/fooz/barz')
            .fail(stubCallback);

         waitsFor(function () {
            return !!stubCallback.calls.length;
         }, 'the request to fail', 30*1000)

         runs( function() {
            expect(stubCallback).toHaveBeenGivenAnyError();
         });
      })
      */

      it('throws node callback errors in a separate event loop', function () {

         var callbackError = new Error('I am a bad callback');
             stubCallback = jasmine.createSpy('error callback').andThrow(callbackError);

         spyOn(globalContext, 'setTimeout');
         oboe(testUrl('static/json/firstTenNaturalNumbers.json'))
            .node('!.*', stubCallback)

         waitsFor(function () {
            return !!stubCallback.calls.length;
         }, 'the callback to be called', ASYNC_TEST_TIMEOUT)

         runs( function() {
            expect(globalContext.setTimeout.mostRecentCall.args[0]).toThrow(callbackError)
         });
      })

      it('throws done callback errors in a separate event loop', function () {

         var callbackError = new Error('I am a bad callback');
            stubCallback = jasmine.createSpy('error callback').andThrow(callbackError);

         spyOn(globalContext, 'setTimeout');
         oboe(testUrl('static/json/firstTenNaturalNumbers.json'))
            .done(stubCallback)

         waitsFor(function () {
            return !!stubCallback.calls.length;
         }, 'the request to fail', ASYNC_TEST_TIMEOUT)


         runs( function() {
            expect(globalContext.setTimeout.mostRecentCall.args[0]).toThrow(callbackError)
         });
      })

      it('can load an empty response with 204 status code', function() {

         var status;

         oboe({
            url: testUrl('204noData')
         })
         .start(function(s){
            status = s;
         })
         .done(whenDoneFn);

         waitsFor(doneCalled, 'the request to have called done', ASYNC_TEST_TIMEOUT);

         runs(function() {
            expect(fullResponse).toEqual({});
            expect(status).toBe(204);
         })
      });

      it('emits error with incomplete json', function () {

         var stubCallback = jasmine.createSpy('error callback');

         oboe(testUrl('static/json/incomplete.json'))
            .fail(stubCallback);

         waitsFor(function () {
            return !!stubCallback.calls.length;
         }, 'the request to fail', ASYNC_TEST_TIMEOUT);
      })

      if( !Platform.isNode ) {
         // only worry about this in the browser

         it( 'hasn\'t put clarinet in the global namespace', function(){

            expect( window.clarinet ).toBeUndefined();
         });
      }

      function someSecondsToPass(waitSecs) {

         function now() {
            // IE8 doesn't have Date.now()
            return new Date().getTime();
         }

         var waitStart = now(),
             waitMs = waitSecs * 1000;

         return function () {
            return now() > (waitStart + waitMs);
         }
      }

      var callbackSpy,
          fullResponse,
          aborted;

      function abortCallback() {
         this.abort();
         aborted = true;
      }

      function theRequestToBeAborted() {
         return aborted
      }

      function whenDoneFn(obj) {
         fullResponse = obj;
      }

      function doneCalled(){
         return !!fullResponse
      }

      /**
       * A context-type header can come back like:
       *    'application/json'
       *
       * or like:
       *    'application/json; charset=UTF-8'
       */
      function contextTypeFrom(header){
         return header.split(';')[0];
      }

      beforeEach(function () {
         aborted = false;
         fullResponse = null;
         callbackSpy = jasmine.createSpy('callbackSpy');

         this.addMatchers({
            toHaveBeenGivenAnyError:function(){
               var errorReport = this.actual.mostRecentCall.args[0],
                   maybeErr = errorReport.thrown;

               if( typeof maybeErr != 'object' ) {
                  return false;
               }

               if( maybeErr instanceof Error ) {
                  return true;
               }

               // instanceof Error doesn't always work in Safari (tested v6.0.5)
               // because:
               //
               // ((new XMLHttpRequestException()) instanceof Error) == false
               if( window && window.XMLHttpRequestException &&
                        maybeErr instanceof XMLHttpRequestException ) {
                  return true;
               }

               // if that didn't work fallback to some duck typing:
               return(  (typeof maybeErr.message != 'undefined') &&
                        (typeof maybeErr.lineNumber != 'undefined') );
            },
            toHaveBeenGivenThrowee:function(expectedError){
               var errorReport = this.actual.mostRecentCall.args[0];

               return errorReport.thrown === expectedError;
            },
            toHaveBeenGivenErrorStatusCode:function(expectedCode){
               var errorReport = this.actual.mostRecentCall.args[0];

               return errorReport.statusCode === expectedCode;
            },
            toHaveBeenGivenBodyJson:function(expectedBodyJson){
               var errorReport = this.actual.mostRecentCall.args[0];

               return JSON.stringify(expectedBodyJson)
                      ===
                      JSON.stringify(errorReport.jsonBody);
            }
         });
      });

   });
})(typeof Platform == 'undefined'? require('../libs/platform.js') : Platform)
